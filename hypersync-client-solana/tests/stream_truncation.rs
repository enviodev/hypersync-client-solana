//! Regression test for HOS-1834: `stream_arrow` must re-request the tail of a
//! server-truncated chunk instead of silently dropping it.
//!
//! The server here simulates row/time caps by advancing at most a fixed number
//! of slots per response, whatever range the request asks for.

use std::sync::Arc;

use hypersync_client_solana::config::{ClientConfig, StreamConfig};
use hypersync_client_solana::Client;
use hypersync_solana_net_types::query::SolanaQuery;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Minimal valid wire body: next_slot, no rollback guard, no tables.
fn wire_body(next_slot: u64) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&next_slot.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b
}

/// Pull the first integer following `key` out of a JSON request body.
fn json_u64(body: &str, key: &str) -> u64 {
    let start = body.find(key).expect("key in request body") + key.len();
    let digits: String = body[start..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().expect("integer after key")
}

/// HTTP server that answers every query with
/// `next_slot = min(to_slot, from_slot + trunc)`: a server that always
/// truncates after `trunc` slots.
async fn spawn_truncating_server(trunc: u64) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));

    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                // Handle sequential requests on this connection (keep-alive).
                let mut buf: Vec<u8> = Vec::new();
                loop {
                    // Read until we hold a full head + body.
                    let (body_start, content_length) = loop {
                        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&buf[..pos + 4]).to_lowercase();
                            let len = head
                                .lines()
                                .find_map(|l| l.strip_prefix("content-length:"))
                                .and_then(|v| v.trim().parse::<usize>().ok())
                                .unwrap_or(0);
                            if buf.len() >= pos + 4 + len {
                                break (pos + 4, len);
                            }
                        }
                        let mut chunk = [0u8; 4096];
                        let read = stream.read(&mut chunk).await.unwrap_or(0);
                        if read == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..read]);
                    };

                    let body = String::from_utf8_lossy(&buf[body_start..body_start + content_length])
                        .to_string();
                    buf.drain(..body_start + content_length);

                    let from = json_u64(&body, "\"from_slot\":");
                    let to = json_u64(&body, "\"to_slot\":");
                    let resp_body = wire_body(to.min(from + trunc));
                    let head = format!(
                        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n",
                        resp_body.len()
                    );
                    if stream.write_all(head.as_bytes()).await.is_err() {
                        return;
                    }
                    if stream.write_all(&resp_body).await.is_err() {
                        return;
                    }
                    let _ = stream.flush().await;
                }
            });
        }
    });

    url
}

#[tokio::test]
async fn truncated_chunks_are_fully_drained() {
    // Server truncates every response after 25 slots. Two concurrent chunks of
    // 50 slots each cover [0, 100), so every chunk needs two round trips.
    let url = spawn_truncating_server(25).await;
    let client = Arc::new(
        Client::new(ClientConfig {
            url,
            bearer_token: None,
            ..Default::default()
        })
        .expect("client"),
    );

    let query = SolanaQuery {
        from_slot: 0,
        to_slot: Some(100),
        ..Default::default()
    };
    let config = StreamConfig {
        batch_size: 50,
        max_batch_size: 50,
        min_batch_size: 50,
        concurrency: 2,
        ..Default::default()
    };

    let mut rx = client.stream_arrow(query, config);
    let mut next_slots = Vec::new();
    while let Some(result) = rx.recv().await {
        next_slots.push(result.expect("page").next_slot);
    }

    // Before the fix this yielded [25, 75, 100]: the second chunk's first
    // response overwrote the cursor and [25, 50) was silently dropped.
    assert_eq!(next_slots, vec![25, 50, 75, 100]);
}

#[tokio::test]
async fn single_chunk_truncation_is_drained_in_order() {
    let url = spawn_truncating_server(10).await;
    let client = Arc::new(
        Client::new(ClientConfig {
            url,
            bearer_token: None,
            ..Default::default()
        })
        .expect("client"),
    );

    let query = SolanaQuery {
        from_slot: 0,
        to_slot: Some(40),
        ..Default::default()
    };
    let config = StreamConfig {
        batch_size: 40,
        max_batch_size: 40,
        min_batch_size: 40,
        concurrency: 4,
        ..Default::default()
    };

    let mut rx = client.stream_arrow(query, config);
    let mut next_slots = Vec::new();
    while let Some(result) = rx.recv().await {
        next_slots.push(result.expect("page").next_slot);
    }

    assert_eq!(next_slots, vec![10, 20, 30, 40]);
}
