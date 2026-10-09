//! https://github.com/enviodev/hyperindex/issues/1691
#![cfg(target_os = "linux")]

use std::time::Duration;

use hypersync_client_solana::config::ClientConfig;
use hypersync_client_solana::Client;
use tokio::net::TcpSocket;

#[tokio::test(start_paused = true)]
async fn unreachable_address_fails_at_connect_timeout() {
    // Linux drops SYNs once the accept queue is full, like a blackholed address.
    let socket = TcpSocket::new_v4().expect("socket");
    socket.bind("127.0.0.1:0".parse().unwrap()).expect("bind");
    let listener = socket.listen(0).expect("listen").into_std().expect("std");
    let addr = listener.local_addr().expect("local addr");
    // Filled with blocking connects: the paused clock would expire a timeout
    // before a loopback handshake had the chance to complete.
    let mut fillers = Vec::new();
    while let Ok(stream) = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(50)) {
        fillers.push(stream);
    }

    let request_timeout = Duration::from_secs(60);
    let client = Client::new(ClientConfig {
        url: format!("http://{addr}"),
        http_req_timeout: request_timeout,
        max_num_retries: 0,
        ..ClientConfig::default()
    })
    .expect("client");
    // The paused clock jumps to whichever timeout fires first.
    let start = tokio::time::Instant::now();
    let res = client.get_height().await;
    assert!(
        res.is_err() && start.elapsed() < request_timeout,
        "{res:?} after {:?}",
        start.elapsed()
    );
}
