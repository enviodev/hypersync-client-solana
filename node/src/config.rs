use std::time::Duration;

use hypersync_client_solana::config::ClientConfig as RustClientConfig;

/// Configuration for the Solana HyperSync client.
#[napi(object)]
#[derive(Default, Clone)]
pub struct ClientConfig {
    /// Base URL of the HyperSync server (e.g. "https://solana.hypersync.xyz").
    pub url: String,
    /// Bearer token for authenticated requests.
    pub bearer_token: Option<String>,
    /// Milliseconds to wait for a response before timing out. Default: 30000.
    pub http_req_timeout_millis: Option<i64>,
    /// Maximum number of retries per request. Default: 12.
    pub max_num_retries: Option<i64>,
    /// Initial backoff before the first retry, in milliseconds. Default: 500.
    pub retry_base_ms: Option<i64>,
    /// Maximum backoff between retries, in milliseconds. Default: 5000.
    pub retry_ceiling_ms: Option<i64>,
    /// Whether to proactively sleep when the rate limit is exhausted instead
    /// of sending requests that will be rejected with 429. Default: true.
    pub proactive_rate_limit_sleep: Option<bool>,
}

impl From<ClientConfig> for RustClientConfig {
    fn from(c: ClientConfig) -> Self {
        let default = RustClientConfig::default();
        RustClientConfig {
            url: c.url,
            bearer_token: c.bearer_token,
            http_req_timeout: c
                .http_req_timeout_millis
                .map(|v| Duration::from_millis(v.max(0) as u64))
                .unwrap_or(default.http_req_timeout),
            max_num_retries: c
                .max_num_retries
                .map(|v| v.max(0) as u32)
                .unwrap_or(default.max_num_retries),
            retry_base_ms: c
                .retry_base_ms
                .map(|v| v.max(0) as u64)
                .unwrap_or(default.retry_base_ms),
            retry_ceiling_ms: c
                .retry_ceiling_ms
                .map(|v| v.max(0) as u64)
                .unwrap_or(default.retry_ceiling_ms),
            proactive_rate_limit_sleep: c
                .proactive_rate_limit_sleep
                .unwrap_or(default.proactive_rate_limit_sleep),
        }
    }
}
