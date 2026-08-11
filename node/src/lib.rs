#[macro_use]
extern crate napi_derive;

mod config;
mod query;
mod types;

use std::sync::Arc;

use anyhow::Context;
use hypersync_client_solana::Client as RsClient;
use hypersync_solana_net_types::query::SolanaQuery as RsSolanaQuery;

use crate::config::ClientConfig;
use crate::query::SolanaQuery;
use crate::types::{QueryResponse, QueryResponseWithRateLimit};

/// Solana HyperSync client.
#[napi]
pub struct SolanaClient {
    inner: Arc<RsClient>,
}

#[napi]
impl SolanaClient {
    /// Create a new client with the given config.
    #[napi(constructor)]
    pub fn new(cfg: ClientConfig) -> napi::Result<SolanaClient> {
        let inner = RsClient::new(cfg.into())
            .context("build Solana HyperSync client")
            .map_err(map_err)?;
        Ok(SolanaClient {
            inner: Arc::new(inner),
        })
    }

    /// Create a new client with the given config and a custom user agent.
    ///
    /// Mirrors the EVM HyperSync client's `newWithAgent`.
    #[napi(factory)]
    pub fn new_with_agent(cfg: ClientConfig, user_agent: String) -> napi::Result<SolanaClient> {
        let inner = RsClient::new_with_agent(cfg.into(), user_agent)
            .context("build Solana HyperSync client")
            .map_err(map_err)?;
        Ok(SolanaClient {
            inner: Arc::new(inner),
        })
    }

    /// Get the current chain height (latest slot).
    #[napi]
    pub async fn get_height(&self) -> napi::Result<i64> {
        let height = self.inner.get_height().await.map_err(map_err)?;
        i64::try_from(height)
            .context("height does not fit in i64")
            .map_err(map_err)
    }

    /// Run a single query and return the decoded response.
    ///
    /// The server may return fewer slots than the range you requested;
    /// inspect `response.nextSlot` to know where to continue from.
    ///
    /// Named to match the EVM HyperSync client's `get`, so `get` and
    /// `getWithRateLimit` read as the pair they are.
    #[napi]
    pub async fn get(&self, query: SolanaQuery) -> napi::Result<QueryResponse> {
        let rs_query: RsSolanaQuery = query.try_into().map_err(map_err)?;
        let resp = self
            .inner
            .get_arrow(&rs_query)
            .await
            .context("run query")
            .map_err(map_err)?;
        QueryResponse::try_from(resp)
            .context("convert response")
            .map_err(map_err)
    }

    /// Run a single query and return the decoded response along with rate
    /// limit information from the server.
    ///
    /// Retry and back-off behaviour is identical to `get`: a 429 is slept out
    /// against `x-ratelimit-reset` and retried. Named and shaped to match the
    /// EVM client's `getWithRateLimit`.
    #[napi]
    pub async fn get_with_rate_limit(
        &self,
        query: SolanaQuery,
    ) -> napi::Result<QueryResponseWithRateLimit> {
        let rs_query: RsSolanaQuery = query.try_into().map_err(map_err)?;
        let res = self
            .inner
            .get_arrow_with_rate_limit(&rs_query)
            .await
            .context("run query")
            .map_err(map_err)?;
        Ok(QueryResponseWithRateLimit {
            response: QueryResponse::try_from(res.response)
                .context("convert response")
                .map_err(map_err)?,
            rate_limit: res.rate_limit.into(),
        })
    }

    /// Get the most recently observed rate limit information.
    /// Returns null if no query response has included rate limit headers yet.
    #[napi]
    pub fn rate_limit_info(&self) -> Option<crate::types::RateLimitInfo> {
        self.inner.rate_limit_info().map(Into::into)
    }

    /// Wait until the current rate limit window resets.
    /// Returns immediately if no rate limit info has been observed or quota remains.
    #[napi]
    pub async fn wait_for_rate_limit(&self) {
        self.inner.wait_for_rate_limit().await;
    }
}

fn map_err(e: anyhow::Error) -> napi::Error {
    napi::Error::from_reason(format!("{:?}", e))
}
