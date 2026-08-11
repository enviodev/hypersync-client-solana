pub mod arrow_reader;
pub mod config;
pub mod decode;
pub mod from_arrow;
pub mod rate_limit;
pub mod simple_types;
pub mod stream;
pub mod types;

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use config::{ClientConfig, StreamConfig};
use hypersync_solana_net_types::query::SolanaQuery;
pub use rate_limit::{QueryResponseWithRateLimit, RateLimitInfo};
use simple_types::SolanaResponse;
use types::QueryResponse;

/// Solana HyperSync client.
///
/// Thread-safe and cheap to clone (wraps an `Arc`).
#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

struct ClientInner {
    http: reqwest::Client,
    base_url: String,
    config: ClientConfig,
    /// Last observed rate limit headers and when they were captured, for the
    /// proactive sleep in [`Client::wait_for_rate_limit`].
    rate_limit_state: std::sync::Mutex<Option<(RateLimitInfo, Instant)>>,
}

/// Outcome of a single query POST, before retry logic.
enum PostError {
    /// Server responded with 429 Too Many Requests.
    RateLimited(RateLimitInfo),
    /// Any other request or server error.
    Other(anyhow::Error),
}

impl Client {
    /// Create a new client with the given configuration.
    pub fn new(config: ClientConfig) -> Result<Self> {
        // hscs stands for hypersync client solana.
        let user_agent = format!("hscs/{}", env!("CARGO_PKG_VERSION"));
        Self::new_with_agent(config, user_agent)
    }

    /// Create a new client with the given configuration and a custom user agent.
    ///
    /// This mirrors the EVM and Fuel HyperSync clients and is intended for use by
    /// language bindings (Node.js) and downstream tools that want to identify
    /// themselves to the server.
    pub fn new_with_agent(config: ClientConfig, user_agent: impl Into<String>) -> Result<Self> {
        anyhow::ensure!(!config.url.is_empty(), "url must not be empty");

        let mut builder = reqwest::Client::builder()
            .timeout(config.http_req_timeout)
            .user_agent(user_agent.into());

        if let Some(ref token) = config.bearer_token {
            use reqwest::header;
            let mut headers = header::HeaderMap::new();
            let val = header::HeaderValue::from_str(&format!("Bearer {}", token))
                .context("invalid bearer token")?;
            headers.insert(header::AUTHORIZATION, val);
            builder = builder.default_headers(headers);
        }

        let http = builder.build().context("build HTTP client")?;
        let base_url = config.url.trim_end_matches('/').to_owned();

        Ok(Self {
            inner: Arc::new(ClientInner {
                http,
                base_url,
                config,
                rate_limit_state: std::sync::Mutex::new(None),
            }),
        })
    }

    /// Get the current chain height (latest slot).
    pub async fn get_height(&self) -> Result<u64> {
        let url = format!("{}/height", self.inner.base_url);
        let resp = self
            .request_with_retry(|| self.inner.http.get(&url))
            .await
            .context("get height")?;

        let text = resp.text().await.context("read height response")?;
        text.trim()
            .parse()
            .with_context(|| format!("parse height '{}'", text.trim()))
    }

    /// Execute a single query and return Arrow data.
    pub async fn get_arrow(&self, query: &SolanaQuery) -> Result<QueryResponse> {
        Ok(self.get_arrow_with_rate_limit(query).await?.response)
    }

    /// Executes query with retries and returns the response in Arrow format
    /// along with rate limit information from the server.
    ///
    /// This is useful for consumers that want to inspect rate limit headers and
    /// implement their own rate limiting logic in external systems. Retry and
    /// back-off behaviour is identical to [`get_arrow`](Self::get_arrow):
    /// a 429 is slept out against `x-ratelimit-reset` and retried.
    pub async fn get_arrow_with_rate_limit(
        &self,
        query: &SolanaQuery,
    ) -> Result<QueryResponseWithRateLimit<QueryResponse>> {
        let (resp_bytes, rate_limit) = self.post_with_retry(query).await.context("query arrow")?;

        let response =
            arrow_reader::decode_response(&resp_bytes).context("decode arrow response")?;
        Ok(QueryResponseWithRateLimit {
            response,
            rate_limit,
        })
    }

    /// Executes query with retries and returns typed Rust structs along with
    /// rate limit information from the server.
    ///
    /// This is useful for consumers that want to inspect rate limit headers and
    /// implement their own rate limiting logic in external systems. Retry and
    /// back-off behaviour is identical to [`get`](Self::get).
    pub async fn get_with_rate_limit(
        &self,
        query: &SolanaQuery,
    ) -> Result<QueryResponseWithRateLimit<SolanaResponse>> {
        let result = self.get_arrow_with_rate_limit(query).await?;
        Ok(QueryResponseWithRateLimit {
            response: decode_response_tables(result.response)?,
            rate_limit: result.rate_limit,
        })
    }

    /// Execute a query that may span many server responses, paginating automatically.
    /// Returns a single merged response.
    pub async fn collect_arrow(
        self: &Arc<Self>,
        query: SolanaQuery,
        config: StreamConfig,
    ) -> Result<QueryResponse> {
        let mut rx = self.stream_arrow(query, config);
        let mut acc: Option<QueryResponse> = None;

        while let Some(result) = rx.recv().await {
            let resp = result?;
            acc = Some(match acc {
                None => resp,
                Some(mut a) => {
                    a.next_slot = resp.next_slot;
                    a.response_bytes += resp.response_bytes;
                    // Keep the most recent page's guard; a page without one
                    // (head not in memory) must not erase an earlier guard
                    // that still covers merged rows.
                    if resp.rollback_guard.is_some() {
                        a.rollback_guard = resp.rollback_guard;
                    }
                    for (name, batch) in resp.data.tables {
                        if batch.num_rows() == 0 {
                            continue;
                        }
                        if let Some(existing) = a.data.tables.get(name) {
                            if existing.num_rows() > 0 {
                                let merged = arrow::compute::concat_batches(
                                    &existing.schema(),
                                    &[existing.clone(), batch],
                                )
                                .with_context(|| format!("concat {} batches", name))?;
                                a.data.tables.insert(name, merged);
                            } else {
                                a.data.tables.insert(name, batch);
                            }
                        } else {
                            a.data.tables.insert(name, batch);
                        }
                    }
                    a
                }
            });
        }

        acc.ok_or_else(|| anyhow::anyhow!("no data returned"))
    }

    /// Stream query results with concurrent fetching and adaptive batch sizing.
    ///
    /// Returns an mpsc receiver that yields `QueryResponse` items in slot order.
    pub fn stream_arrow(
        self: &Arc<Self>,
        query: SolanaQuery,
        config: StreamConfig,
    ) -> mpsc::Receiver<Result<QueryResponse>> {
        stream::stream_arrow(self.clone(), query, config)
    }

    /// Execute a single query and return typed Rust structs.
    ///
    /// Like [`Client::get_arrow`], but decodes the Arrow tables into the
    /// `Vec<T>` shapes in [`crate::simple_types`].
    pub async fn get(&self, query: &SolanaQuery) -> Result<SolanaResponse> {
        let arrow = self.get_arrow(query).await?;
        decode_response_tables(arrow)
    }

    /// Execute a query that may span many server responses, paginating
    /// automatically, and return typed Rust structs.
    ///
    /// This is the typed counterpart of [`Client::collect_arrow`].
    pub async fn collect(
        self: &Arc<Self>,
        query: SolanaQuery,
        config: StreamConfig,
    ) -> Result<SolanaResponse> {
        let arrow = self.collect_arrow(query, config).await?;
        decode_response_tables(arrow)
    }

    /// Executes the query POST once. 429 responses become
    /// [`PostError::RateLimited`] with the parsed rate limit headers; every
    /// successful response's headers are returned so callers can track their
    /// quota.
    async fn post_once(
        &self,
        query: &SolanaQuery,
    ) -> std::result::Result<(Vec<u8>, RateLimitInfo), PostError> {
        let url = format!("{}/query/arrow", self.inner.base_url);
        let resp = self
            .inner
            .http
            .post(&url)
            .json(query)
            .send()
            .await
            .map_err(|e| PostError::Other(anyhow::Error::from(e).context("execute http req")))?;

        let status = resp.status();
        let rate_limit = RateLimitInfo::from_response(&resp);

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(PostError::RateLimited(rate_limit));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(PostError::Other(anyhow::anyhow!(
                "query returned {}: {}",
                status,
                body
            )));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| PostError::Other(anyhow::Error::from(e).context("read response body")))?;
        Ok((bytes.to_vec(), rate_limit))
    }

    /// Executes the query with retries.
    ///
    /// A 429 sleeps until the window resets and retries; other transient errors
    /// use the generic exponential back-off. Once the retries are spent, the
    /// caller receives the accumulated attempt errors.
    async fn post_with_retry(&self, query: &SolanaQuery) -> Result<(Vec<u8>, RateLimitInfo)> {
        let cfg = &self.inner.config;
        let mut err = anyhow::anyhow!("");

        // Proactive throttling: if we know we're rate limited, wait before sending.
        if cfg.proactive_rate_limit_sleep {
            self.wait_for_rate_limit().await;
        }

        for attempt in 0..=cfg.max_num_retries {
            match self.post_once(query).await {
                Ok((bytes, rate_limit)) => {
                    self.update_rate_limit_state(&rate_limit);
                    return Ok((bytes, rate_limit));
                }
                Err(PostError::RateLimited(rate_limit)) => {
                    self.update_rate_limit_state(&rate_limit);
                    err = err.context(format!(
                        "rate limited by server ({rate_limit}). To increase your rate limits, upgrade your plan at https://envio.dev/app/api-tokens"
                    ));
                    if attempt == cfg.max_num_retries {
                        return Err(err);
                    }

                    let wait_secs = rate_limit.suggested_wait_secs().unwrap_or(1) + 1;
                    tracing::warn!(
                        attempt,
                        %rate_limit,
                        wait_secs,
                        "rate limited by server, waiting before retry. To increase your rate limits, upgrade your plan at https://envio.dev/app/api-tokens. For more info: https://docs.envio.dev/docs/HyperSync/api-tokens"
                    );
                    tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                    continue;
                }
                Err(PostError::Other(e)) => {
                    tracing::warn!(attempt, error = ?e, "Query failed");
                    err = err.context(format!("{e:?}"));
                    if attempt == cfg.max_num_retries {
                        return Err(err);
                    }

                    let delay_ms = cfg.retry_base_ms * 2u64.pow((attempt + 1).min(5));
                    let delay_ms = delay_ms.min(cfg.retry_ceiling_ms);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }

        Err(err)
    }

    /// Locks the rate limit state, recovering from a poisoned mutex.
    ///
    /// The state is advisory metadata, so a stale value is always preferable to
    /// bricking every later call on a client whose lock holder happened to panic.
    fn lock_rate_limit_state(&self) -> std::sync::MutexGuard<'_, Option<(RateLimitInfo, Instant)>> {
        self.inner
            .rate_limit_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Returns the most recently observed rate limit information, if any.
    ///
    /// Updated after successful and 429 query responses that contain rate limit
    /// headers.
    pub fn rate_limit_info(&self) -> Option<RateLimitInfo> {
        self.lock_rate_limit_state()
            .as_ref()
            .map(|(info, _captured_at)| info.clone())
    }

    /// Waits until the current rate limit window resets, if the client is rate limited.
    ///
    /// Returns immediately if:
    /// - No rate limit information has been observed yet
    /// - There is remaining quota in the current window
    ///
    /// This method is useful for consumers who want to explicitly wait before making
    /// requests, for example when coordinating rate limits across multiple systems.
    pub async fn wait_for_rate_limit(&self) {
        let wait_info = {
            let state = self.lock_rate_limit_state();
            match state.as_ref() {
                Some((info, captured_at)) if info.is_rate_limited() => {
                    info.suggested_wait_secs().map(|secs| {
                        let elapsed = captured_at.elapsed().as_secs();
                        let remaining_wait = secs.saturating_sub(elapsed);
                        (remaining_wait, info.clone())
                    })
                }
                _ => None,
            }
        };
        if let Some((secs, info)) = wait_info {
            if secs > 0 {
                tracing::warn!(
                    rate_limit = %info,
                    wait_secs = secs,
                    "rate limit exhausted, proactively waiting for window reset. To increase your rate limits, upgrade your plan at https://envio.dev/app/api-tokens. For more info: https://docs.envio.dev/docs/HyperSync/api-tokens"
                );
                tokio::time::sleep(Duration::from_secs(secs)).await;
            }
        }
    }

    /// Updates the internally tracked rate limit state with the current timestamp.
    fn update_rate_limit_state(&self, rate_limit: &RateLimitInfo) {
        // Only update if the response actually contained rate limit headers
        if rate_limit.limit.is_some()
            || rate_limit.remaining.is_some()
            || rate_limit.reset_secs.is_some()
        {
            *self.lock_rate_limit_state() = Some((rate_limit.clone(), Instant::now()));
        }
    }

    async fn request_with_retry<F>(&self, make_request: F) -> Result<reqwest::Response>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let cfg = &self.inner.config;
        let mut last_err = None;

        for attempt in 0..=cfg.max_num_retries {
            if attempt > 0 {
                let delay_ms = cfg.retry_base_ms * 2u64.pow(attempt.min(5));
                let delay_ms = delay_ms.min(cfg.retry_ceiling_ms);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            match make_request().send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return Ok(resp);
                    }
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    tracing::warn!(attempt, status = %status, "Request failed");
                    last_err = Some(anyhow::anyhow!("HTTP {}: {}", status, body));
                }
                Err(e) => {
                    tracing::warn!(attempt, error = ?e, "Request error");
                    last_err = Some(e.into());
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("request failed after retries")))
    }
}

fn decode_response_tables(arrow: QueryResponse) -> Result<SolanaResponse> {
    let mut resp = SolanaResponse {
        next_slot: arrow.next_slot,
        rollback_guard: arrow.rollback_guard,
        response_bytes: arrow.response_bytes,
        ..Default::default()
    };
    for (name, batch) in arrow.data.tables {
        match name {
            "blocks" => {
                resp.blocks = from_arrow::blocks_from_arrow(&batch).context("decode blocks")?
            }
            "transactions" => {
                resp.transactions =
                    from_arrow::transactions_from_arrow(&batch).context("decode transactions")?
            }
            "instruction_calls" => {
                resp.instruction_calls = from_arrow::instruction_calls_from_arrow(&batch)
                    .context("decode instruction_calls")?
            }
            "logs" => resp.logs = from_arrow::logs_from_arrow(&batch).context("decode logs")?,
            "account_activity" => {
                resp.account_activity = from_arrow::account_activity_from_arrow(&batch)
                    .context("decode account_activity")?
            }
            "rewards" => {
                resp.rewards = from_arrow::rewards_from_arrow(&batch).context("decode rewards")?
            }
            other => {
                tracing::debug!(table = other, "ignoring unknown table in response");
            }
        }
    }
    Ok(resp)
}
