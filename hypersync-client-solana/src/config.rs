use std::time::Duration;

/// Configuration for the Solana HyperSync client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Base URL of the HyperSync server (e.g. "https://solana.hypersync.xyz").
    pub url: String,
    /// Bearer token for authenticated requests.
    pub bearer_token: Option<String>,
    /// Timeout for individual HTTP requests.
    pub http_req_timeout: Duration,
    /// Maximum number of retries per request.
    pub max_num_retries: u32,
    /// Base delay for exponential backoff retries.
    pub retry_base_ms: u64,
    /// Maximum delay for exponential backoff retries.
    pub retry_ceiling_ms: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            bearer_token: None,
            http_req_timeout: Duration::from_secs(30),
            max_num_retries: 12,
            retry_base_ms: 500,
            retry_ceiling_ms: 5_000,
        }
    }
}

/// Configuration for concurrent streaming.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Number of slots to request per chunk.
    pub batch_size: u64,
    /// Maximum batch size (for adaptive sizing).
    pub max_batch_size: u64,
    /// Minimum batch size (for adaptive sizing).
    pub min_batch_size: u64,
    /// Number of concurrent in-flight requests.
    pub concurrency: usize,
    /// Response byte threshold above which batch size shrinks.
    pub response_bytes_ceiling: u64,
    /// Response byte threshold below which batch size grows.
    pub response_bytes_floor: u64,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            batch_size: 1_000,
            max_batch_size: 200_000,
            min_batch_size: 100,
            concurrency: 10,
            response_bytes_ceiling: 500_000,
            response_bytes_floor: 250_000,
        }
    }
}
