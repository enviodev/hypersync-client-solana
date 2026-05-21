//! Live decoder smoke test against `solana.hypersync.xyz`.
//!
//! Pulls ~200 recent Metaplex Token Metadata instructions, runs each
//! through the bundled schema, and asserts that any
//! `CreateMetadataAccountV3` we see has non-null `name`/`symbol`/`uri`
//! strings (the only payload invariant we can rely on across NFTs).
//!
//! Gated `#[ignore]` so CI doesn't depend on the network. Run with:
//!     cargo test -p hypersync-client-solana --test decode_live -- --ignored --nocapture

use std::sync::Arc;
use std::time::Duration;

use hypersync_client_solana::config::{ClientConfig, StreamConfig};
use hypersync_client_solana::decode::{decode_instruction, metaplex_token_metadata, DecodeError};
use hypersync_client_solana::Client;
use hypersync_solana_net_types::query::{InstructionSelection, SolanaQuery};

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
    };
    Arc::new(Client::new(cfg).expect("build client"))
}

#[tokio::test]
#[ignore]
async fn decode_recent_metaplex_instructions() {
    let client = make_client();
    let height = client.get_height().await.expect("get_height");

    let from = height.saturating_sub(10_000);
    let q = SolanaQuery {
        from_slot: from,
        to_slot: Some(height),
        instructions: vec![InstructionSelection {
            program_id: vec![TOKEN_METADATA_PROGRAM.to_string()],
            include_transaction: false,
            include_logs: false,
            ..Default::default()
        }],
        max_num_instructions: Some(200),
        ..Default::default()
    };

    let resp = client
        .collect(q, StreamConfig::default())
        .await
        .expect("collect");

    assert!(
        !resp.instructions.is_empty(),
        "expected at least one Metaplex instruction in the last 10k slots"
    );
    eprintln!("pulled {} instructions", resp.instructions.len());

    let schema = metaplex_token_metadata();
    let mut decoded_ok = 0usize;
    let mut unknown_disc = 0usize;
    let mut other_err = 0usize;
    let mut create_v3_seen = 0usize;

    for ix in &resp.instructions {
        match decode_instruction(schema, ix) {
            Ok(d) => {
                decoded_ok += 1;
                if d.name == "CreateMetadataAccountV3" {
                    create_v3_seen += 1;
                    let data = d.args.get("data").expect("data field");
                    for key in ["name", "symbol", "uri"] {
                        let v = data.get(key).expect("DataV2 field");
                        assert!(v.is_string(), "{key} should be string, got {v:?}");
                    }
                }
            }
            Err(DecodeError::UnknownDiscriminator(_)) => unknown_disc += 1,
            Err(e) => {
                other_err += 1;
                eprintln!("  decode error on disc {:02x?}: {e}", &ix.data[..1]);
            }
        }
    }

    eprintln!(
        "decoded {} / {} (unknown_disc={}, other_err={}, create_v3_seen={})",
        decoded_ok,
        resp.instructions.len(),
        unknown_disc,
        other_err,
        create_v3_seen
    );

    // The bundled schema only covers 6 of ~50 instructions, so a high
    // unknown-disc rate is expected. We assert at least *something* decoded
    // and that no non-discriminator error surfaced (decoder soundness).
    assert!(decoded_ok > 0, "no instructions decoded");
    assert_eq!(
        other_err, 0,
        "decoder produced non-disc errors on real data"
    );
}
