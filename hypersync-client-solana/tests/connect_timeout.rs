//! https://github.com/enviodev/hyperindex/issues/1691
#![cfg(target_os = "linux")]

use std::time::Duration;

use hypersync_client_solana::config::ClientConfig;
use hypersync_client_solana::Client;
use tokio::net::TcpSocket;

/// The client's private connect timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test(start_paused = true)]
async fn unreachable_address_fails_at_connect_timeout() {
    // Linux drops SYNs once the accept queue is full, like a blackholed address.
    let socket = TcpSocket::new_v4().expect("socket");
    socket.bind("127.0.0.1:0".parse().unwrap()).expect("bind");
    let listener = socket.listen(0).expect("listen").into_std().expect("std");
    let addr = listener.local_addr().expect("local addr");
    // Filled with blocking connects: the paused clock would expire a timeout
    // before a loopback handshake had the chance to complete. It has to end on
    // a timed out connect, or the address refuses instead of dropping and the
    // request below would fail without any timeout.
    let mut fillers = Vec::new();
    loop {
        match std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(50)) {
            Ok(stream) => fillers.push(stream),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => panic!("the address must drop connections, not fail them: {e}"),
        }
    }

    let client = Client::new(ClientConfig {
        url: format!("http://{addr}"),
        http_req_timeout: Duration::from_secs(60),
        max_num_retries: 0,
        ..ClientConfig::default()
    })
    .expect("client");
    // The paused clock jumps to whichever timeout fires first, so the elapsed
    // time names the timer.
    let start = tokio::time::Instant::now();
    let res = client.get_height().await;
    let elapsed = start.elapsed();
    assert!(
        res.is_err()
            && elapsed >= CONNECT_TIMEOUT
            && elapsed < CONNECT_TIMEOUT + Duration::from_secs(1),
        "{res:?} after {elapsed:?}"
    );
}
