use std::collections::HashMap;

use anyhow::{Context, Result};
use arrow::json::writer::{JsonArray, WriterBuilder};
use arrow::record_batch::RecordBatch;
use hypersync_client_solana::types::QueryResponse as RsQueryResponse;
use serde_json::Value;

/// One row in a returned Solana table. Keys are column names in `snake_case`.
pub type RowObject = HashMap<String, Value>;

/// Reorg guard attached to query responses: the server's in-memory head
/// window (its last block, plus the first slot of the window and that slot's
/// parent hash), so a consumer can detect a fork and unwind before committing.
/// Shape mirrors the EVM client's rollbackGuard with Solana naming.
#[napi(object)]
pub struct RollbackGuard {
    /// The last slot in the server's in-memory window.
    pub slot_number: i64,
    /// Timestamp of the last block.
    pub timestamp: i64,
    /// Blockhash of the last block (base58).
    pub blockhash: String,
    /// The first slot in the server's in-memory window.
    pub first_slot_number: i64,
    /// Previous blockhash of the first block in the window (base58).
    pub first_previous_blockhash: String,
}

/// Response from a Solana HyperSync query.
#[napi(object)]
pub struct QueryResponse {
    /// Next slot to query from (for pagination).
    pub next_slot: i64,
    /// Number of bytes in the raw server response (useful for tuning).
    pub response_bytes: i64,
    /// Reorg guard describing the server's in-memory head window, when the
    /// server has one to report.
    pub rollback_guard: Option<RollbackGuard>,
    /// Per-table row arrays. Keys are table names: `blocks`, `transactions`,
    /// `instruction_calls`, `logs`, `account_activity`, `rewards`.
    pub tables: HashMap<String, Vec<RowObject>>,
}

impl TryFrom<RsQueryResponse> for QueryResponse {
    type Error = anyhow::Error;

    fn try_from(resp: RsQueryResponse) -> Result<Self> {
        let mut tables = HashMap::with_capacity(resp.data.tables.len());
        for (name, batch) in resp.data.tables {
            let rows = record_batch_to_rows(&batch)
                .with_context(|| format!("convert {} to rows", name))?;
            tables.insert(name.to_string(), rows);
        }

        let rollback_guard = resp
            .rollback_guard
            .map(|g| -> Result<RollbackGuard> {
                Ok(RollbackGuard {
                    slot_number: g
                        .slot_number
                        .try_into()
                        .context("slot_number does not fit in i64")?,
                    timestamp: g.timestamp,
                    blockhash: g.blockhash.to_string(),
                    first_slot_number: g
                        .first_slot_number
                        .try_into()
                        .context("first_slot_number does not fit in i64")?,
                    first_previous_blockhash: g.first_previous_blockhash.to_string(),
                })
            })
            .transpose()?;

        Ok(QueryResponse {
            next_slot: resp
                .next_slot
                .try_into()
                .context("next_slot does not fit in i64")?,
            response_bytes: resp
                .response_bytes
                .try_into()
                .context("response_bytes does not fit in i64")?,
            rollback_guard,
            tables,
        })
    }
}

fn record_batch_to_rows(batch: &RecordBatch) -> Result<Vec<RowObject>> {
    if batch.num_rows() == 0 {
        return Ok(Vec::new());
    }
    let mut buf = Vec::with_capacity(batch.num_rows() * 64);
    let mut writer = WriterBuilder::new()
        .with_explicit_nulls(false)
        .build::<_, JsonArray>(&mut buf);
    writer.write(batch).context("write batch as JSON")?;
    writer.finish().context("finish JSON writer")?;
    let rows: Vec<RowObject> =
        serde_json::from_slice(&buf).context("parse arrow-JSON output as row objects")?;
    Ok(rows)
}

/// Rate limit information from response headers. Shape mirrors the EVM
/// client's RateLimitInfo.
#[napi(object)]
#[derive(Default)]
pub struct RateLimitInfo {
    /// Total request quota for the current window (`x-ratelimit-limit`).
    pub limit: Option<i64>,
    /// Remaining budget in the current window (`x-ratelimit-remaining`).
    /// Budget units, not request count: divide by `cost` for requests left.
    pub remaining: Option<i64>,
    /// Seconds until the window resets (`x-ratelimit-reset`).
    pub reset_secs: Option<i64>,
    /// Budget consumed per request (`x-ratelimit-cost`).
    pub cost: Option<i64>,
}

impl From<hypersync_client_solana::RateLimitInfo> for RateLimitInfo {
    fn from(info: hypersync_client_solana::RateLimitInfo) -> Self {
        let clamp = |v: Option<u64>| v.and_then(|v| i64::try_from(v).ok());
        Self {
            limit: clamp(info.limit),
            remaining: clamp(info.remaining),
            reset_secs: clamp(info.reset_secs),
            cost: clamp(info.cost),
        }
    }
}

/// Response from `queryWithRateLimit`.
///
/// When the server answers 429, `response` is absent and `rateLimit` carries
/// the back-off headers; the client does NOT retry, so the caller can
/// implement its own pacing.
#[napi(object)]
pub struct QueryWithRateLimitResponse {
    /// The decoded response; absent when the request was rate limited.
    pub response: Option<QueryResponse>,
    /// Rate limit information from response headers (present either way).
    pub rate_limit: RateLimitInfo,
}
