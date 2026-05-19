# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.2-rc.1] - 2026-05-19

Release candidate cut to unblock the hyperindex Solana ecosystem integration.

### Added

- `simple_types` module: typed `Block`, `Transaction`, `Instruction`, `Log`,
  `Balance`, `TokenBalance`, `Reward`, and a `SolanaResponse` bundle.
- `from_arrow` module: decode an Arrow `RecordBatch` into the typed structs
  above (column-by-column, name-based lookup).
- `Client::get` and `Client::collect`: typed counterparts of `get_arrow` /
  `collect_arrow`, returning `SolanaResponse` directly.
- Unit tests covering each table's Arrow → struct round-trip.
- `#[ignore]`-gated live smoke test against `solana.hypersync.xyz` that filters
  on the Metaplex Token Metadata program and asserts non-zero instruction count.
- PLAN.md: new Phase 7b for a Borsh instruction decoder API
  (Anchor IDL + declarative schema for pre-Anchor programs).

## [0.0.1] - 2026-05-18

First crates.io release. All three Rust crates published at this version
(`hypersync-solana-schema`, `hypersync-solana-net-types`, `hypersync-client-solana`).

### Added

- Cargo workspace at the repo root covering all three Rust crates plus a
  Node.js binding crate under `node/`.
- MPL-2.0 LICENSE, README, CHANGELOG.
- Per-crate metadata (description, license, repository, homepage, keywords,
  categories, readme).
- napi-rs Node bindings under `node/` exposing `SolanaClient` with
  `getHeight()` and `query()`.

### Fixed

- Client `Cargo.toml` path dependencies now resolve inside this repo when used
  standalone (previously only resolved as a path dep from the main hypersync repo).
