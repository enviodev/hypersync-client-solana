//! Behavior tests for the rate-limit surface, against a local scripted HTTP
//! server (no network beyond loopback).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hypersync_client_solana::config::{ClientConfig, StreamConfig};
use hypersync_client_solana::Client;
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
            let Some(body_start) = body_start else {
                continue;
            };
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

            let n = hits_srv.fetch_add(1, Ordering::SeqCst);
            let (status, headers, body) = {
                let script = script.lock().expect("script mutex");
                script
                    .get(n)
                    .or_else(|| script.last())
                    .expect("script must not be empty")
                    .clone()
            };

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
    make_client_with_retries(url, proactive_rate_limit_sleep, 3)
}

fn make_client_with_retries(
    url: String,
    proactive_rate_limit_sleep: bool,
    max_num_retries: u32,
) -> Client {
    Client::new(ClientConfig {
        url,
        http_req_timeout: Duration::from_secs(5),
        max_num_retries,
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
async fn with_rate_limit_retries_a_429_and_returns_the_quota_headers() {
    // Same retry semantics as the plain path, matching the EVM client: the 429
    // is slept out and retried, and the caller gets the response plus quota.
    let script = vec![
        (
            429,
            vec![("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "0")],
            Vec::new(),
        ),
        (
            200,
            vec![
                ("x-ratelimit-limit", "50, 50;w=60"),
                ("x-ratelimit-remaining", "40"),
                ("x-ratelimit-cost", "10"),
            ],
            empty_response_body(),
        ),
    ];
    let (url, hits) = spawn_server(script).await;
    let client = make_client(url, true);

    let result = client
        .get_arrow_with_rate_limit(&SolanaQuery::default())
        .await
        .expect("the 429 must be retried, not surfaced");
    assert_eq!(
        (
            result.response.next_slot,
            result.rate_limit.limit,
            result.rate_limit.remaining,
            result.rate_limit.cost,
            hits.load(Ordering::SeqCst),
        ),
        (7, Some(50), Some(40), Some(10), 2),
        "the returned quota must come from the response that actually succeeded"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn exhausted_retries_surface_the_rate_limit_message() {
    let (url, hits) = spawn_server(vec![(429, rate_limit_headers(), Vec::new())]).await;
    // No retries: the single 429 is all the client gets.
    let client = make_client_with_retries(url, true, 0);

    let err = tokio::time::timeout(
        Duration::from_secs(1),
        client.get_arrow_with_rate_limit(&SolanaQuery::default()),
    )
    .await
    .expect("the final attempt must not sleep when no retry remains")
    .err()
    .expect("a 429 that outlives the retries must be an error");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("rate limited by server") && msg.contains("remaining=0/5 reqs"),
        "the error must name the quota so operators can act on it, got: {msg}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn accumulated_error_includes_every_failed_attempt() {
    let script = vec![
        (500, Vec::new(), b"first failure".to_vec()),
        (500, Vec::new(), b"second failure".to_vec()),
    ];
    let (url, hits) = spawn_server(script).await;
    let client = make_client_with_retries(url, true, 1);

    let err = client
        .get_arrow(&SolanaQuery::default())
        .await
        .err()
        .expect("both attempts fail");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("first failure") && msg.contains("second failure"),
        "the caller must receive context from every attempt, got: {msg}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn proactive_sleep_delays_the_next_request_until_the_window_resets() {
    let script = vec![
        (
            429,
            vec![("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1")],
            Vec::new(),
        ),
        (200, Vec::new(), empty_response_body()),
    ];
    let (url, hits) = spawn_server(script).await;
    let client = make_client_with_retries(url, true, 0);

    // Time from before the request that records the window. The client's clock
    // starts when the 429 lands, and the remaining wait is computed in whole
    // seconds: if more than a second passes here the window has genuinely
    // elapsed and a correct client skips the wait entirely.
    let window_started = std::time::Instant::now();
    assert!(
        client.get_arrow(&SolanaQuery::default()).await.is_err(),
        "first call observes the exhausted window"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // The window has not elapsed, so the next call must wait it out before
    // sending rather than spending a request on a certain 429.
    client
        .get_arrow(&SolanaQuery::default())
        .await
        .expect("second call succeeds after the window resets");
    assert!(
        window_started.elapsed() >= Duration::from_secs(1),
        "the second call must wait out the reset window before sending"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn no_proactive_sleep_when_disabled() {
    let (url, hits) = spawn_server(vec![(429, rate_limit_headers(), Vec::new())]).await;
    // reset=30 in the headers: with proactive sleep on, the second call would
    // block for ~30s instead of returning.
    let client = make_client_with_retries(url, false, 0);

    for _ in 0..2 {
        tokio::time::timeout(
            Duration::from_secs(1),
            client.get_arrow(&SolanaQuery::default()),
        )
        .await
        .expect("with proactive sleep disabled no call may block on the window")
        .err()
        .expect("server is still returning 429");
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "with proactive sleep disabled every call must reach the server"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn wait_for_rate_limit_waits_only_for_the_active_window() {
    let headers = vec![("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1")];
    let (url, _hits) = spawn_server(vec![(429, headers, Vec::new())]).await;
    let client = make_client_with_retries(url, false, 0);

    // Time from before the request that records the window, not after: the
    // client's clock starts when the 429 lands, so measuring later can demand
    // more than the wait a correct implementation owes us.
    let window_started = std::time::Instant::now();
    assert!(
        client.get_arrow(&SolanaQuery::default()).await.is_err(),
        "observe rate limit"
    );

    client.wait_for_rate_limit().await;
    assert!(window_started.elapsed() >= Duration::from_secs(1));

    tokio::time::timeout(Duration::from_millis(100), client.wait_for_rate_limit())
        .await
        .expect("elapsed window must not be waited twice");
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
    let (response, rate_limit) = (result.response, result.rate_limit);
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

#[tokio::test(flavor = "multi_thread")]
async fn success_without_headers_preserves_the_last_observed_quota() {
    let script = vec![
        (
            200,
            vec![
                ("x-ratelimit-limit", "50, 50;w=60"),
                ("x-ratelimit-remaining", "40"),
                ("x-ratelimit-reset", "12"),
            ],
            empty_response_body(),
        ),
        (200, Vec::new(), empty_response_body()),
    ];
    let (url, hits) = spawn_server(script).await;
    let client = make_client(url, true);

    client
        .get_arrow(&SolanaQuery::default())
        .await
        .expect("first call");
    client
        .get_arrow(&SolanaQuery::default())
        .await
        .expect("second call");

    let tracked = client
        .rate_limit_info()
        .expect("a headerless success must not erase the previous quota");
    assert_eq!(
        (tracked.limit, tracked.remaining, tracked.reset_secs),
        (Some(50), Some(40), Some(12))
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn cost_only_headers_do_not_create_rate_limit_state() {
    let script = vec![(200, vec![("x-ratelimit-cost", "10")], empty_response_body())];
    let (url, _hits) = spawn_server(script).await;
    let client = make_client(url, true);

    let result = client
        .get_arrow_with_rate_limit(&SolanaQuery::default())
        .await
        .expect("call");
    assert_eq!(result.rate_limit.cost, Some(10));
    assert!(
        client.rate_limit_info().is_none(),
        "cost alone does not count as rate limit state in the EVM client"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn non_429_errors_do_not_update_rate_limit_state() {
    let (url, _hits) = spawn_server(vec![(500, rate_limit_headers(), b"failed".to_vec())]).await;
    let client = make_client_with_retries(url, true, 0);

    client
        .get_arrow(&SolanaQuery::default())
        .await
        .err()
        .expect("server error");
    assert!(client.rate_limit_info().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn stream_pages_use_the_rate_limit_retry_path() {
    let script = vec![
        (
            429,
            vec![("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "0")],
            Vec::new(),
        ),
        (200, Vec::new(), empty_response_body()),
    ];
    let (url, hits) = spawn_server(script).await;
    let client = Arc::new(make_client(url, true));
    let query = SolanaQuery {
        from_slot: 0,
        to_slot: Some(1),
        ..Default::default()
    };
    let mut stream = client.stream_arrow(
        query,
        StreamConfig {
            concurrency: 1,
            ..Default::default()
        },
    );

    let response = tokio::time::timeout(Duration::from_secs(3), stream.recv())
        .await
        .expect("stream page must retry without generic backoff")
        .expect("stream response")
        .expect("successful page");
    assert_eq!(response.next_slot, 7);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_height_does_not_participate_in_rate_limit_state_tracking() {
    let (url, hits) = spawn_server(vec![(429, rate_limit_headers(), Vec::new())]).await;
    let client = make_client_with_retries(url, true, 1);

    client.get_height().await.expect_err("height request fails");
    assert!(client.rate_limit_info().is_none());
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "get_height uses generic retries, but none may update query rate-limit state"
    );
}
