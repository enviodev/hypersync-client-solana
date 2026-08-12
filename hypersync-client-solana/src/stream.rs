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
/// Splits the slot range into disjoint chunks and fetches them concurrently.
/// Each chunk is paginated to completion by its own worker: the server is free
/// to truncate any response (row/time caps, `next_slot < to_slot`), and the
/// worker keeps re-requesting from `next_slot` until its chunk is fully
/// covered. Pages are forwarded chunk-by-chunk, so overall slot ordering is
/// preserved. Batch size adapts to response byte counts.
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
            // Fill a window of concurrent chunk workers.
            let mut chunks = Vec::with_capacity(config.concurrency);
            let mut chunk_end = current;

            for _ in 0..config.concurrency {
                if chunk_end >= to {
                    break;
                }
                let end = cmp::min(to, chunk_end + batch_size);
                let mut q = query.clone();
                q.from_slot = chunk_end;
                q.to_slot = Some(end);

                // Small per-chunk buffer: workers ahead of the forwarding
                // cursor can prefetch a little without unbounded memory use.
                let (chunk_tx, chunk_rx) = mpsc::channel::<Result<QueryResponse>>(2);
                let client = client.clone();
                let handle = tokio::spawn(async move {
                    let mut cur = q.from_slot;
                    let chunk_to = q.to_slot.expect("chunk to_slot is always set");
                    // Drain the whole chunk. A response that ends short of
                    // chunk_to was truncated by the server; continue from
                    // next_slot instead of dropping the tail (HOS-1834).
                    while cur < chunk_to {
                        q.from_slot = cur;
                        match client.get_arrow(&q).await {
                            Ok(resp) => {
                                if resp.next_slot <= cur {
                                    let _ = chunk_tx
                                        .send(Err(anyhow::anyhow!(
                                            "server made no progress at slot {cur}"
                                        )))
                                        .await;
                                    return;
                                }
                                cur = resp.next_slot;
                                if chunk_tx.send(Ok(resp)).await.is_err() {
                                    // Receiver dropped; stop fetching.
                                    return;
                                }
                            }
                            Err(e) => {
                                let _ = chunk_tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                });

                chunks.push((chunk_rx, handle));
                chunk_end = end;
            }

            if chunks.is_empty() {
                break;
            }

            // Forward pages chunk-by-chunk to maintain slot ordering.
            for (mut chunk_rx, handle) in chunks {
                while let Some(result) = chunk_rx.recv().await {
                    if let Ok(resp) = &result {
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

                    let is_err = result.is_err();
                    if tx.send(result).await.is_err() {
                        tracing::warn!("Stream receiver dropped");
                        return;
                    }
                    if is_err {
                        return;
                    }
                }

                // Channel closed: surface a worker panic instead of silently
                // treating its chunk as complete.
                if let Err(e) = handle.await {
                    let _ = tx.send(Err(anyhow::anyhow!("chunk task failed: {}", e))).await;
                    return;
                }
            }
        }
    });

    rx
}
