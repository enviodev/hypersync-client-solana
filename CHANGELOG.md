# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.8] - 2026-07-21

Solana query-layer Wave 2. Every rename keeps the legacy key working via a
serde alias (Rust) or a deprecated fallback field (node), so existing queries
and JS callers are unaffected.

### Added

- `net-types` / `node`: `TransactionSelection.transaction_id` (base58
  `signatures[0]`, the canonical Solana transaction signature) and
  `transaction_index` filter arrays, plus `TransactionField::TransactionId` so
  the signature id can be selected.
- `net-types`: `InstructionField::ExecutingAccountIndex` and
  `AccountIndexArguments` columns.
- `net-types` / `node`: `InstructionSelection.is_committed` (`isCommitted` in
  JS), an optional tri-state filter
  on the commit status of the parent transaction. `None` / absent matches both
  committed and failed transactions (existing behavior), `Some(true)` matches
  only instructions of successful transactions, `Some(false)` only those of
  failed transactions. Mirrors the existing `is_inner` filter, and complements
  the already-selectable `InstructionField::IsCommitted` response field.

  Failed Solana transactions still land on chain and their instructions are
  served, so consumers that count effects (token transfers, swaps) over-count
  without this filter. Requires a server that understands the new key: older
  servers ignore it and return instructions of failed transactions regardless,
  so clients that need the guarantee should also filter on the response
  `is_committed` field.

### Changed

- `net-types` / `node`: `SolanaQuery.instructions` renamed to
  `instruction_calls`, and `SolanaFieldSelection.instruction` to
  `instruction_call`. One row is one runtime program invocation (an execution
  trace, including CPIs), the Solana counterpart to EVM traces.
- `net-types` / `node`: `InstructionSelection.program_id` renamed to
  `executing_account`, `InstructionField::ProgramId` to `ExecutingAccount`, and
  `InstructionField::Accounts` to `AccountArguments`.
- `node`: on every renamed key the new name wins when both are supplied; the
  old name is marked `@deprecated` and still honored when the new one is
  absent.

### Fixed

- `node`: the napi bindings and the two client integration tests had not been
  updated for the renames, so `cargo check --workspace --all-targets` failed.
  Regenerating `index.d.ts` also restored `BalanceSelection`,
  `TokenBalanceSelection`, `includeBalances` / `includeTokenBalances` and the
  balance row caps, which had drifted out of sync with the Rust types.
- `node`: `transaction_index` is `u64` on the wire but napi only carries `i64`,
  so the conversion now rejects negative values instead of wrapping.

## [0.0.7] - 2026-06-11

### Removed

- `net-types` / `node`: the per-selection `include_*` join flags on
  `InstructionSelection` (`include_transaction`, `include_logs`,
  `include_inner_instructions`, `include_balances`, `include_token_balances`),
  `TransactionSelection` (`include_instructions`, `include_balances`,
  `include_token_balances`), and `LogSelection` (`include_transaction`,
  `include_instruction`, `include_balances`, `include_token_balances`). The
  server no longer handles them — it always applies the single default join
  driven by `field_selection`. Queries that still carry these keys keep
  deserializing (the unknown keys are ignored). The top-level
  `SolanaQuery.include_balances` / `include_token_balances` result-set flags and
  `include_all_blocks` are unaffected.
- `net-types` / `node`: the legacy `fields` alias for `field_selection`. Only
  `field_selection` is accepted now, matching the EVM and Fuel HyperSync query
  APIs.

### Added

- `client` / `node`: `Client::new_with_agent` (Rust) and
  `SolanaClient.createWithAgent` (Node) to construct a client with a custom user
  agent, mirroring the EVM and Fuel HyperSync clients.

## [0.0.6] - 2026-06-09

First official release of the query-API consistency change. Consolidates the
0.0.6-rc.1..rc.3 candidates. Backwards compatible.

### Changed

- `net-types` / `node`: `SolanaQuery.fields` is renamed to `field_selection`
  (matching the EVM and Fuel HyperSync query APIs). The legacy `fields` key is
  still accepted on input (serde alias on the wire; deprecated `fields` on the
  node client), so existing queries keep working.

### Notes

- There is no configurable join mode (the `join_mode` API explored in rc.2 was
  removed in rc.3). The server applies a single default join.
- The per-selection `include_*` join flags are retained for backwards
  compatibility and are still honored by the server today; they are slated to
  become no-ops once the server moves to the single default join, and a future
  version may reject them.

## [0.0.6-rc.3] - 2026-06-09

Removes the `join_mode` API added in rc.2. The team decided against
configurable join modes: the server will always apply a single default join
(related rows are returned based on `field_selection`). This is additive to
remove since nothing depends on rc.2's `join_mode` yet.

### Removed

- `net-types`: `JoinMode` enum and `SolanaQuery.join_mode`.
- `node`: `SolanaQuery.joinMode`.

### Notes

- The per-selection `include_*` flags remain accepted (and still deserialize)
  but are documented as ignored no-ops; the server always applies the default
  join. A future version may reject them.
- `field_selection` and the legacy `fields` alias are unchanged.

## [0.0.6-rc.2] - 2026-06-09

Adds the `join_mode` query API. Forward-compatible: queries that omit it get
`Default`, and the legacy per-selection `include_*` join flags still
deserialize (the server will treat them as no-ops once it adopts join modes).

### Added

- `net-types`: `JoinMode` enum (`JoinNothing` / `Linear` / `Default` / `JoinAll`,
  variant names matching EVM HyperSync, with `Linear` added) and a
  `SolanaQuery.join_mode` field (default `Default`).

### Changed

- `net-types`: the per-selection `include_*` flags are documented as a
  transition shim - accepted on input but slated to become server-side no-ops
  driven by `join_mode`; a future version may reject them.

## [0.0.6-rc.1] - 2026-06-09

Release candidate for the `fields` -> `field_selection` rename. Backwards
compatible: the legacy key keeps working on both the wire and the node client.

### Changed

- `net-types`: rename `SolanaQuery.fields` to `field_selection`, matching the EVM
  and Fuel HyperSync query APIs. A serde `alias = "fields"` keeps deserializing
  the legacy key, so existing JSON queries are unaffected; serialization now
  emits `field_selection`.
- `node`: add `fieldSelection` to `SolanaQuery`; `fields` is retained as a
  deprecated alias (if both are set, `fieldSelection` wins).

## [0.0.4] - 2026-05-29

Solana balances "data gap" fix (HOS-1298): decouple balances/token_balances
from `include_all_blocks` and add a transaction-scoped balance join, so a
value-flow / DEX indexer can get balance deltas for the transactions its
instruction/transaction/log filter already matched without pulling every block
in the range. All additions are optional `#[serde(default)]` fields, so existing
queries and responses are unchanged (non-breaking).

### Added

- `net-types`: top-level `SolanaQuery.include_balances` /
  `include_token_balances` flags that return balances for the matched result set
  without `include_all_blocks` (with no other filters: all balances in range, SQD
  parity).
- `net-types`: per-selection `include_balances` / `include_token_balances` join
  flags on `InstructionSelection`, `TransactionSelection`, and `LogSelection`.
  When set, the server returns only the balance rows whose
  `(slot, transaction_index)` is in the matched set (strict improvement over
  SQD, which returns all-block balances).
- `net-types`: `BalanceSelection { account }` and
  `TokenBalanceSelection { account, mint, owner, program_id }` request-filter
  objects, plus `SolanaQuery.balances` / `token_balances`,
  `max_num_balances` / `max_num_token_balances`.
- `schema`: `token_balance` gains `pre_program_id` / `post_program_id` columns
  (classic SPL Token vs Token-2022). Amounts were already decimal strings, so
  Token-2022 amounts above `u64::MAX` already round-trip.
- `field_selection`: `TokenBalanceField::PreProgramId` / `PostProgramId`.
- client: `TokenBalance` gains `pre_program_id` / `post_program_id`; the Arrow
  decoder reads them as optional columns (older servers still decode).
- node bindings updated to expose all of the above.

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
