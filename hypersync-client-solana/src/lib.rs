pub mod arrow_reader;
pub mod config;
pub mod stream;
pub mod types;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use config::{ClientConfig, StreamConfig};
use hypersync_solana_net_types::query::SolanaQuery;
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
}

impl Client {
    /// Create a new client with the given configuration.
    pub fn new(config: ClientConfig) -> Result<Self> {
        anyhow::ensure!(!config.url.is_empty(), "url must not be empty");

        let mut builder = reqwest::Client::builder().timeout(config.http_req_timeout);

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
        let url = format!("{}/query/arrow", self.inner.base_url);

        let resp_bytes = self
            .post_with_retry(&url, query)
            .await
            .context("query arrow")?;

        arrow_reader::decode_response(&resp_bytes).context("decode arrow response")
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

    async fn post_with_retry(&self, url: &str, query: &SolanaQuery) -> Result<Vec<u8>> {
        let cfg = &self.inner.config;
        let mut last_err = None;

        for attempt in 0..=cfg.max_num_retries {
            if attempt > 0 {
                let delay_ms = cfg.retry_base_ms * 2u64.pow(attempt.min(5));
                let delay_ms = delay_ms.min(cfg.retry_ceiling_ms);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            match self.inner.http.post(url).json(query).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        let body = resp.text().await.unwrap_or_default();
                        let err = anyhow::anyhow!("query returned {}: {}", status, body);
                        tracing::warn!(attempt, status = %status, "Query failed");
                        last_err = Some(err);
                        continue;
                    }
                    let bytes = resp.bytes().await.context("read response body")?;
                    return Ok(bytes.to_vec());
                }
                Err(e) => {
                    tracing::warn!(attempt, error = ?e, "Query request failed");
                    last_err = Some(e.into());
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("query failed after retries")))
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
