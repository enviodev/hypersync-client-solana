//! https://github.com/enviodev/hyperindex/issues/1691
#![cfg(target_os = "linux")]

use std::time::Duration;

use hypersync_client_solana::config::ClientConfig;
use hypersync_client_solana::Client;
use tokio::net::{TcpSocket, TcpStream};

#[tokio::test]
async fn unreachable_address_fails_at_connect_timeout() {
    // Linux drops SYNs once the accept queue is full, like a blackholed address.
    let socket = TcpSocket::new_v4().expect("socket");
    socket.bind("127.0.0.1:0".parse().unwrap()).expect("bind");
    let listener = socket.listen(0).expect("listen");
    let addr = listener.local_addr().expect("local addr");
    let mut fillers = Vec::new();
    while let Ok(Ok(stream)) =
        tokio::time::timeout(Duration::from_millis(500), TcpStream::connect(addr)).await
    {
        fillers.push(stream);
    }

    let client = Client::new(ClientConfig {
        url: format!("http://{addr}"),
        http_req_timeout: Duration::from_secs(60),
        max_num_retries: 0,
        ..ClientConfig::default()
    })
    .expect("client");
    let res = tokio::time::timeout(Duration::from_secs(7), client.get_height()).await;
    assert!(matches!(res, Ok(Err(_))), "{res:?}");
}
