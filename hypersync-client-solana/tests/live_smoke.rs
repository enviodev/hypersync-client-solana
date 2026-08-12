//! Live smoke test against `solana.hypersync.xyz`.
//!
//! Gated by `#[ignore]` so CI does not depend on the network. Run with:
//!     cargo test -p hypersync-client-solana --test live_smoke -- --ignored --nocapture

use std::sync::Arc;
use std::time::Duration;

use hypersync_client_solana::config::{ClientConfig, StreamConfig};
use hypersync_client_solana::Client;
use hypersync_solana_net_types::query::{InstructionSelection, SolanaQuery};

/// Metaplex Token Metadata program.
const TOKEN_METADATA_PROGRAM: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

fn make_client() -> Arc<Client> {
    let cfg = ClientConfig {
        url: std::env::var("HYPERSYNC_SOLANA_URL")
            .unwrap_or_else(|_| "https://solana.hypersync.xyz".to_string()),
        bearer_token: std::env::var("HYPERSYNC_SOLANA_TOKEN").ok(),
        http_req_timeout: Duration::from_secs(60),
        max_num_retries: 3,
        retry_base_ms: 500,
        retry_ceiling_ms: 5_000,
        ..Default::default()
    };
    Arc::new(Client::new(cfg).expect("build client"))
}

#[tokio::test]
#[ignore]
async fn get_height_returns_recent_slot() {
    let client = make_client();
    let height = client.get_height().await.expect("get_height");
    eprintln!("current slot: {}", height);
    assert!(height > 300_000_000, "height looks too low: {}", height);
}

#[tokio::test]
#[ignore]
async fn collect_returns_token_metadata_instructions() {
    let client = make_client();
    let height = client.get_height().await.expect("get_height");

    // Look back ~10k slots. Token Metadata is heavily used so this window
    // almost always contains at least one instruction.
    let from = height.saturating_sub(10_000);
    let to = height;
    eprintln!("querying slots [{}, {})", from, to);

    let q = SolanaQuery {
        from_slot: from,
        to_slot: Some(to),
        instruction_calls: vec![InstructionSelection {
            executing_account: vec![TOKEN_METADATA_PROGRAM.parse().unwrap()],
            ..Default::default()
        }],
        // Cap the response so we don't pull megabytes on every run.
        max_num_instructions: Some(200),
        ..Default::default()
    };

    let resp = client
        .collect(q, StreamConfig::default())
        .await
        .expect("collect");

    eprintln!(
        "got {} instructions, {} transactions, {} blocks, next_slot={}",
        resp.instruction_calls.len(),
        resp.transactions.len(),
        resp.blocks.len(),
        resp.next_slot
    );
    assert!(
        !resp.instruction_calls.is_empty(),
        "expected at least one Token Metadata instruction"
    );
    for ix in resp.instruction_calls.iter().take(3) {
        assert_eq!(
            ix.executing_account.map(|a| a.to_string()).as_deref(),
            Some(TOKEN_METADATA_PROGRAM)
        );
        assert!(
            ix.data.as_deref().is_some_and(|d| !d.is_empty()),
            "instruction data should not be empty"
        );
    }
}
