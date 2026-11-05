//! Query-id cache behaviour against a local mock server: ids are only sent
//! once a server has issued one, a miss falls back to the full body within
//! the same call, and servers without the cache see only full bodies.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use hypersync_client_solana::config::ClientConfig;
use hypersync_client_solana::Client;
use hypersync_solana_net_types::query::SolanaQuery;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const ID: &str = "abababababababababababababababababababababababababababababababab";

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Issues an id on store requests and serves it on by-id.
    Caching,
    /// Issues ids but forgets them (a pod restart, eviction or another shard).
    Forgetful,
    /// Pre-cache server: ignores the store header, has no by-id route.
    Old,
    /// Issues ids, but by-id fails with a 503 the first time.
    ByIdUnavailableOnce,
    /// Issues ids, but by-id is rejected with 401 (e.g. an expired token).
    ByIdUnauthorized,
}

/// One received request: path, whether it asked to store, and the JSON body.
#[derive(Debug, Clone)]
struct Seen {
    path: String,
    store: bool,
    body: serde_json::Value,
}

fn empty_response_body() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&7u64.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b
}

async fn spawn_server(mode: Mode) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::<Seen>::new()));
    let seen_srv = seen.clone();

    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
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
            let path = head
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("")
                .to_owned();
            let store = head
                .lines()
                .any(|l| l.trim() == "x-hypersync-query-cache: store");
            let body: serde_json::Value =
                serde_json::from_slice(&buf[body_start..]).unwrap_or_default();

            let by_id_count = {
                let mut seen = seen_srv.lock().unwrap();
                seen.push(Seen {
                    path: path.clone(),
                    store,
                    body,
                });
                seen.iter().filter(|s| s.path.ends_with("/by-id")).count()
            };

            let (status, headers): (u16, Vec<(&str, &str)>) = match (mode, path.as_str()) {
                (Mode::Old, "/query/arrow") => (200, vec![]),
                (Mode::Old, _) => (404, vec![]),
                (_, "/query/arrow") if store => (200, vec![("x-hypersync-query-id", ID)]),
                (_, "/query/arrow") => (200, vec![]),
                (Mode::Forgetful, _) => (404, vec![("x-hypersync-query-cache", "miss")]),
                (Mode::ByIdUnavailableOnce, _) if by_id_count == 1 => (503, vec![]),
                (Mode::ByIdUnauthorized, _) => (401, vec![]),
                _ => (200, vec![]),
            };
            let body = if status == 200 {
                empty_response_body()
            } else {
                b"{}".to_vec()
            };
            let mut resp = format!("HTTP/1.1 {status} X\r\n");
            for (name, value) in headers {
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
    (url, seen)
}

fn client(url: String, enable_query_cache: bool) -> Client {
    Client::new(ClientConfig {
        url,
        http_req_timeout: Duration::from_secs(5),
        max_num_retries: 3,
        retry_base_ms: 10,
        retry_ceiling_ms: 50,
        enable_query_cache,
        ..Default::default()
    })
    .unwrap()
}

fn page(from_slot: u64) -> SolanaQuery {
    SolanaQuery {
        from_slot,
        to_slot: Some(from_slot + 10),
        include_all_blocks: true,
        ..Default::default()
    }
}

fn paths(seen: &Mutex<Vec<Seen>>) -> Vec<String> {
    seen.lock()
        .unwrap()
        .iter()
        .map(|s| s.path.clone())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn later_pages_send_the_issued_id_with_their_own_range() {
    let (url, seen) = spawn_server(Mode::Caching).await;
    let client = client(url, true);
    for from in [0, 10, 20] {
        client.get_arrow(&page(from)).await.unwrap();
    }
    assert_eq!(
        paths(&seen),
        ["/query/arrow", "/query/arrow/by-id", "/query/arrow/by-id"]
    );
    let seen = seen.lock().unwrap();
    assert!(seen[0].store, "first page asks the server to cache");
    assert_eq!(
        seen[2].body,
        serde_json::json!({ "query_id": ID, "from_slot": 20, "to_slot": 30 })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_different_query_gets_its_own_full_body() {
    let (url, seen) = spawn_server(Mode::Caching).await;
    let client = client(url, true);
    client.get_arrow(&page(0)).await.unwrap();
    let other = SolanaQuery {
        include_all_blocks: false,
        ..page(0)
    };
    client.get_arrow(&other).await.unwrap();
    assert_eq!(paths(&seen), ["/query/arrow", "/query/arrow"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_miss_falls_back_to_the_full_body_in_the_same_call() {
    let (url, seen) = spawn_server(Mode::Forgetful).await;
    let client = client(url, true);
    client.get_arrow(&page(0)).await.unwrap();
    client.get_arrow(&page(10)).await.unwrap();
    assert_eq!(
        paths(&seen),
        ["/query/arrow", "/query/arrow/by-id", "/query/arrow"]
    );
    let seen = seen.lock().unwrap();
    assert!(seen[2].store, "the fallback re-stores to get a fresh id");
    assert_eq!(seen[2].body["from_slot"], 10);
}

#[tokio::test(flavor = "multi_thread")]
async fn repeated_misses_pause_the_cache_for_that_query() {
    let (url, seen) = spawn_server(Mode::Forgetful).await;
    let client = client(url, true);
    for from in [0, 10, 20, 30, 40] {
        client.get_arrow(&page(from)).await.unwrap();
    }
    let seen = seen.lock().unwrap();
    let trace: Vec<(&str, bool)> = seen.iter().map(|s| (s.path.as_str(), s.store)).collect();
    assert_eq!(
        trace,
        [
            ("/query/arrow", true),
            ("/query/arrow/by-id", false),
            ("/query/arrow", true),
            ("/query/arrow/by-id", false),
            ("/query/arrow", true),
            // Third miss in a row: the fallback stops asking to store...
            ("/query/arrow/by-id", false),
            ("/query/arrow", false),
            // ...and later pages skip the cache: one request each.
            ("/query/arrow", false),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_non_miss_4xx_on_by_id_is_an_error_not_a_second_full_body() {
    let (url, seen) = spawn_server(Mode::ByIdUnauthorized).await;
    let client = client(url, true);
    client.get_arrow(&page(0)).await.unwrap();
    assert!(client.get_arrow(&page(10)).await.is_err());
    let paths = paths(&seen);
    assert_eq!(paths[0], "/query/arrow");
    assert!(
        paths[1..].iter().all(|p| p == "/query/arrow/by-id"),
        "no full-body fallback on 401: {paths:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn old_servers_only_ever_see_full_bodies() {
    let (url, seen) = spawn_server(Mode::Old).await;
    let client = client(url, true);
    for from in [0, 10, 20] {
        client.get_arrow(&page(from)).await.unwrap();
    }
    assert_eq!(
        paths(&seen),
        ["/query/arrow"; 3],
        "no wasted by-id requests"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_error_on_by_id_is_retried_not_treated_as_a_miss() {
    let (url, seen) = spawn_server(Mode::ByIdUnavailableOnce).await;
    let client = client(url, true);
    client.get_arrow(&page(0)).await.unwrap();
    client.get_arrow(&page(10)).await.unwrap();
    assert_eq!(
        paths(&seen),
        ["/query/arrow", "/query/arrow/by-id", "/query/arrow/by-id"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn disabled_cache_sends_no_store_header() {
    let (url, seen) = spawn_server(Mode::Caching).await;
    let client = client(url, false);
    client.get_arrow(&page(0)).await.unwrap();
    client.get_arrow(&page(10)).await.unwrap();
    assert_eq!(paths(&seen), ["/query/arrow"; 2]);
    assert!(seen.lock().unwrap().iter().all(|s| !s.store));
}
