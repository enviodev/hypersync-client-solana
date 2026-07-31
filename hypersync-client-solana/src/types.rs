use std::collections::BTreeMap;

use arrow::record_batch::RecordBatch;
use hypersync_solana_net_types::RollbackGuard;

/// Response from a single query to the HyperSync server.
pub struct QueryResponse {
    /// The next slot to query from (for pagination).
    pub next_slot: u64,
    /// Reorg guard for the scanned range, when the server produced one.
    pub rollback_guard: Option<RollbackGuard>,
    /// Named Arrow tables (e.g. "blocks", "transactions", "instruction_calls").
    pub data: ArrowResponseData,
    /// Number of bytes in the raw response (for adaptive batch sizing).
    pub response_bytes: usize,
}

/// Arrow record batches keyed by table name.
pub struct ArrowResponseData {
    pub tables: BTreeMap<&'static str, RecordBatch>,
}
