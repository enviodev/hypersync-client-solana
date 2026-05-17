# hypersync-client-solana

[![CI](https://github.com/enviodev/hypersync-client-solana/actions/workflows/ci.yaml/badge.svg?branch=main)](https://github.com/enviodev/hypersync-client-solana/actions/workflows/ci.yaml)
<a href="https://crates.io/crates/hypersync-client-solana">
<img src="https://img.shields.io/crates/v/hypersync-client-solana.svg?style=flat-square" alt="Crates.io version" />
</a>

Rust client for [Envio's](https://envio.dev/) Solana HyperSync.

[Documentation page](https://docs.envio.dev/docs/HyperSync/overview)

## Workspace layout

This repository is a Cargo workspace with three crates:

- `hypersync-client-solana` - the high-level client (`Client`, streaming, retry).
- `hypersync-solana-net-types` - request/response types shared with the server.
- `hypersync-solana-schema` - Arrow schemas for the Solana tables.

## Install

```toml
[dependencies]
hypersync-client-solana = "0.1"
```

## Quick start

```rust
use std::sync::Arc;
use hypersync_client_solana::{Client, config::ClientConfig};
use hypersync_solana_net_types::query::SolanaQuery;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Arc::new(Client::new(ClientConfig {
        url: "https://solana.hypersync.xyz".into(),
        bearer_token: std::env::var("HYPERSYNC_BEARER_TOKEN").ok(),
        ..Default::default()
    })?);

    let height = client.get_height().await?;
    println!("current slot: {height}");

    let query = SolanaQuery {
        from_slot: height.saturating_sub(100),
        to_slot: Some(height),
        include_all_blocks: true,
        ..Default::default()
    };
    let resp = client.get_arrow(&query).await?;
    for (name, batch) in &resp.data.tables {
        println!("{name}: {} rows", batch.num_rows());
    }
    Ok(())
}
```

A fluent `Client::builder()` and `SolanaQuery::builder()` are coming in the next
releases (see `PLAN.md`).

## License

[MPL-2.0](LICENSE)
