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
use crate::types::QueryResponse;

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
    /// Mirrors the EVM and Fuel HyperSync clients' `createWithAgent`.
    #[napi(factory)]
    pub fn create_with_agent(cfg: ClientConfig, user_agent: String) -> napi::Result<SolanaClient> {
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
    #[napi]
    pub async fn query(&self, query: SolanaQuery) -> napi::Result<QueryResponse> {
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
}

fn map_err(e: anyhow::Error) -> napi::Error {
    napi::Error::from_reason(format!("{:?}", e))
}
