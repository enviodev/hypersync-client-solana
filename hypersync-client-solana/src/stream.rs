use std::cmp;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::config::StreamConfig;
use crate::types::QueryResponse;
use crate::Client;

use hypersync_solana_net_types::query::SolanaQuery;

/// Streams query results with concurrent fetching and adaptive batch sizing.
///
/// Splits the slot range into chunks, fetches them concurrently, and dynamically
/// adjusts the batch size based on response byte counts.
pub fn stream_arrow(
    client: Arc<Client>,
    query: SolanaQuery,
    config: StreamConfig,
) -> mpsc::Receiver<Result<QueryResponse>> {
    let (tx, rx) = mpsc::channel(config.concurrency * 2);

    tokio::task::spawn(async move {
        let from = query.from_slot;
        let to = query.to_slot.unwrap_or(u64::MAX);

        let mut current = from;
        let mut batch_size = config.batch_size;

        while current < to {
            // Fill a window of concurrent requests
            let mut futs = Vec::with_capacity(config.concurrency);
            let mut chunk_end = current;

            for _ in 0..config.concurrency {
                if chunk_end >= to {
                    break;
                }
                let end = cmp::min(to, chunk_end + batch_size);
                let mut q = query.clone();
                q.from_slot = chunk_end;
                q.to_slot = Some(end);

                let client = client.clone();
                futs.push(tokio::spawn(async move { client.get_arrow(&q).await }));
                chunk_end = end;
            }

            if futs.is_empty() {
                break;
            }

            // Await results in order to maintain slot ordering
            for fut in futs {
                let result = match fut.await {
                    Ok(r) => r,
                    Err(e) => Err(anyhow::anyhow!("task join error: {}", e)),
                };

                match &result {
                    Ok(resp) => {
                        // Adaptive batch sizing based on response bytes
                        let bytes = resp.response_bytes as u64;
                        if bytes > config.response_bytes_ceiling
                            && batch_size > config.min_batch_size
                        {
                            batch_size = cmp::max(config.min_batch_size, batch_size / 2);
                            tracing::debug!(
                                batch_size,
                                response_bytes = bytes,
                                "Shrunk batch size (response too large)"
                            );
                        } else if bytes < config.response_bytes_floor
                            && batch_size < config.max_batch_size
                        {
                            batch_size = cmp::min(config.max_batch_size, batch_size * 2);
                            tracing::debug!(
                                batch_size,
                                response_bytes = bytes,
                                "Grew batch size (response too small)"
                            );
                        }

                        current = resp.next_slot;
                    }
                    Err(_) => {
                        // On error, send it and stop streaming
                    }
                }

                let is_err = result.is_err();
                if tx.send(result).await.is_err() {
                    tracing::warn!("Stream receiver dropped");
                    return;
                }
                if is_err {
                    return;
                }
            }
        }
    });

    rx
}
