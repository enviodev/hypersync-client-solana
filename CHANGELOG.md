# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.3-rc.1] - 2026-05-21

Borsh instruction decoder (PLAN.md Phase 7b). Targeted at hyperindex Stage 7a.

### Added

- `decode` module: schema-driven Borsh interpreter that walks a raw
  `Instruction::data` buffer against a `ProgramSchema` and produces a
  `DecodedInstruction { name, args, named_accounts, extra_accounts }`.
- `decode::schema`: `ProgramSchema`, `InstructionSchema`, `NamedAccount`,
  `NamedField`, `FieldType`, `EnumVariant`, `DefinedTypes`, `DecodeError`.
- `decode::anchor_idl::schema_from_anchor_idl_json`: parser supporting both
  Anchor 0.30+ (inline `discriminator`, `writable`/`signer`) and Anchor 0.29
  (computed discriminator via `sha256("global:<snake_case_name>")[..8]`,
  `isMut`/`isSigner`). Single code path; format detected from the IDL shape.
- `decode::bundled::metaplex_token_metadata`: hand-written `ProgramSchema`
  covering six common instructions on
  `metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s`:
  `CreateMetadataAccount` (0x00), `UpdateMetadataAccountV2` (0x0f),
  `CreateMasterEditionV3` (0x11), `VerifyCollection` (0x12),
  `CreateMetadataAccountV3` (0x21), `Burn` (0x29).
- 23 primitive-level tests, 7 Anchor IDL tests, 7 bundled-Metaplex tests, and
  a `#[ignore]`-gated live test against `solana.hypersync.xyz`.

### Conventions (locked)

- Sub-64-bit integers render as `Value::Number`; `u64`/`i64`/`u128`/`i128`
  render as decimal `Value::String` (downstream converts to `bigint`).
- All 32-byte fields (Anchor `pubkey`, `[u8; 32]`) render as base58 strings.
- Borsh `bytes` (`Vec<u8>`) renders as `0x`-prefixed lowercase hex.
- `extra_accounts` carries accounts beyond the IDL-declared list explicitly
  (Anchor `remaining_accounts`, IDL drift); empty when counts match.

### Notes

- PLAN.md listed VerifyCollection at `0x10` and Burn at `0x2c`; the on-chain
  `MetadataInstruction` variant ordering puts them at `0x12` and `0x29`
  respectively. The shipped schema uses the canonical bytes; PLAN.md should
  be updated to match.

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
