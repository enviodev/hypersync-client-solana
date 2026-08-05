//! Behavior tests for the rate-limit surface, against a local scripted HTTP
//! server (no network beyond loopback).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hypersync_client_solana::config::ClientConfig;
use hypersync_client_solana::{Client, RateLimitResponse};
use hypersync_solana_net_types::query::SolanaQuery;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// One scripted HTTP response: status code, extra headers, body.
type Scripted = (u16, Vec<(&'static str, &'static str)>, Vec<u8>);

/// Minimal valid wire body: next_slot=7, no rollback guard, no tables.
fn empty_response_body() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&7u64.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b
}

/// Serves the scripted responses in order, one connection each, repeating the
/// last script entry if more requests arrive. Returns the base URL and a hit
/// counter.
async fn spawn_server(script: Vec<Scripted>) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("local addr"));
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_srv = hits.clone();
    let script = Arc::new(Mutex::new(script));

    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let n = hits_srv.fetch_add(1, Ordering::SeqCst);
            let (status, headers, body) = {
                let script = script.lock().expect("script mutex");
                script
                    .get(n)
                    .or_else(|| script.last())
                    .expect("script must not be empty")
                    .clone()
            };

            // Drain the request: headers, then content-length body bytes.
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let body_start = loop {
                let read = stream.read(&mut chunk).await.unwrap_or(0);
                if read == 0 {
                    break None;
                }
                buf.extend_from_slice(&chunk[..read]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(pos + 4);
                }
            };
            if let Some(body_start) = body_start {
                let head = String::from_utf8_lossy(&buf[..body_start]).to_lowercase();
                let content_length: usize = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                while buf.len() < body_start + content_length {
                    let read = stream.read(&mut chunk).await.unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..read]);
                }
            }

            let reason = match status {
                200 => "OK",
                429 => "Too Many Requests",
                _ => "Error",
            };
            let mut resp = format!("HTTP/1.1 {status} {reason}\r\n");
            for (name, value) in &headers {
                resp.push_str(&format!("{name}: {value}\r\n"));
            }
            resp.push_str(&format!(
                "content-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            ));
            let _ = stream.write_all(resp.as_bytes()).await;
            let _ = stream.write_all(&body).await;
            let _ = stream.shutdown().await;
        }
    });

    (url, hits)
}

fn make_client(url: String, proactive_rate_limit_sleep: bool) -> Client {
    Client::new(ClientConfig {
        url,
        http_req_timeout: Duration::from_secs(5),
        max_num_retries: 3,
        retry_base_ms: 10,
        retry_ceiling_ms: 50,
        proactive_rate_limit_sleep,
        ..Default::default()
    })
    .expect("build client")
}

fn rate_limit_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("x-ratelimit-limit", "50, 50;w=60"),
        ("x-ratelimit-remaining", "0"),
        ("x-ratelimit-reset", "30"),
        ("x-ratelimit-cost", "10"),
    ]
}

#[tokio::test(flavor = "multi_thread")]
async fn with_rate_limit_returns_429_without_retry_then_skips_proactively() {
    let (url, hits) = spawn_server(vec![(429, rate_limit_headers(), Vec::new())]).await;
    let client = make_client(url, true);

    let result = client
        .get_arrow_with_rate_limit(&SolanaQuery::default())
        .await
        .expect("call");
    let info = match result {
        RateLimitResponse::RateLimited(info) => info,
        RateLimitResponse::Success { .. } => panic!("expected RateLimited"),
    };
    assert_eq!(
        (
            info.limit,
            info.remaining,
            info.reset_secs,
            info.cost,
            hits.load(Ordering::SeqCst),
        ),
        (Some(50), Some(0), Some(30), Some(10), 1),
        "429 must be returned immediately with parsed headers, no retry"
    );

    // The window (30s) has not elapsed, so the next call must be answered from
    // the tracked state without touching the server.
    let result = client
        .get_arrow_with_rate_limit(&SolanaQuery::default())
        .await
        .expect("call");
    assert!(
        matches!(result, RateLimitResponse::RateLimited(_)) && hits.load(Ordering::SeqCst) == 1,
        "second call must be rejected proactively without a server hit"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn with_rate_limit_hits_server_when_proactive_sleep_disabled() {
    let (url, hits) = spawn_server(vec![(429, rate_limit_headers(), Vec::new())]).await;
    let client = make_client(url, false);

    for _ in 0..2 {
        let result = client
            .get_arrow_with_rate_limit(&SolanaQuery::default())
            .await
            .expect("call");
        assert!(matches!(result, RateLimitResponse::RateLimited(_)));
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "with proactive sleep disabled every call must reach the server"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn plain_get_arrow_waits_out_a_429_and_retries() {
    // reset=0 keeps the internal wait at its 1s floor.
    let script = vec![
        (
            429,
            vec![("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "0")],
            Vec::new(),
        ),
        (200, Vec::new(), empty_response_body()),
    ];
    let (url, hits) = spawn_server(script).await;
    let client = Client::new(ClientConfig {
        url,
        http_req_timeout: Duration::from_secs(5),
        max_num_retries: 3,
        // A rate-limit retry must use the server's reset delay, not also pay
        // the generic exponential backoff used for other transient errors.
        retry_base_ms: 10_000,
        retry_ceiling_ms: 10_000,
        proactive_rate_limit_sleep: true,
        ..Default::default()
    })
    .expect("build client");

    let resp = tokio::time::timeout(
        Duration::from_secs(3),
        client.get_arrow(&SolanaQuery::default()),
    )
    .await
    .expect("rate-limit retry must not also use generic retry backoff")
    .expect("get_arrow must retry through the 429");
    assert_eq!(
        (resp.next_slot, hits.load(Ordering::SeqCst)),
        (7, 2),
        "the 429 must be retried once and the success response decoded"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn success_carries_rate_limit_headers() {
    let script = vec![(
        200,
        vec![
            ("x-ratelimit-limit", "50, 50;w=60"),
            ("x-ratelimit-remaining", "40"),
            ("x-ratelimit-reset", "12"),
        ],
        empty_response_body(),
    )];
    let (url, _hits) = spawn_server(script).await;
    let client = make_client(url, true);

    let result = client
        .get_with_rate_limit(&SolanaQuery::default())
        .await
        .expect("call");
    let (response, rate_limit) = match result {
        RateLimitResponse::Success {
            response,
            rate_limit,
        } => (response, rate_limit),
        RateLimitResponse::RateLimited(_) => panic!("expected Success"),
    };
    assert_eq!(
        (
            response.next_slot,
            rate_limit.limit,
            rate_limit.remaining,
            rate_limit.reset_secs,
        ),
        (7, Some(50), Some(40), Some(12)),
        "success must carry both the decoded response and the quota headers"
    );

    let tracked = client
        .rate_limit_info()
        .expect("successful response headers must be tracked");
    assert_eq!(
        (tracked.limit, tracked.remaining, tracked.reset_secs),
        (Some(50), Some(40), Some(12))
    );
}
