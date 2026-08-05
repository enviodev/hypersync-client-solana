# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Rate-limit surface mirroring the EVM `hypersync-client`:
  - `Client::get_with_rate_limit` / `Client::get_arrow_with_rate_limit` return
    a `RateLimitResponse` instead of retrying on HTTP 429, so consumers can
    implement their own back-off. Other transient errors still retry.
  - `RateLimitInfo` parsed from the `x-ratelimit-*` response headers on every
    query response, and `Client::wait_for_rate_limit` to explicitly wait out
    an exhausted window.
  - `ClientConfig::proactive_rate_limit_sleep` (default `true`): skip sending
    requests that would be rejected with 429 while the window is exhausted.
    The `_with_rate_limit` methods return `RateLimited` proactively instead.
  - Node bindings: `queryWithRateLimit`, `rateLimitInfo`,
    `waitForRateLimit`, and the `proactiveRateLimitSleep` config field.

## [0.2.0-rc.4] - 2026-08-01

The Wave 2 API lock: wire names, physical names and client types are aligned
into the shape the server will keep. Everything here rides the full re-sync,
so no legacy mapping layers survive.

### Changed

- **Breaking:** table `instructions` is now `instruction_calls`; columns
  `program_id` -> `executing_account`, `accounts` -> `account_arguments`,
  `is_committed` -> `tx_success` on it. Legacy names are still accepted on
  query input via serde aliases; responses use the new names only.
- **Breaking:** `account_activity.owner` is split into `pre_owner` /
  `post_owner`; the collapsed column is gone.
- **Breaking:** every response-side field is `Option<T>`; addresses, hashes
  and signatures decode into base58 newtypes (`Address`, `Hash`,
  `Signature`) with strict, loud parsing; token balances are u64 base units
  carried as strings (they exceed the JS safe-integer range).
- **Breaking (node):** `includeAccountActivity` now errors with guidance
  (use `accountActivity: [{}]`); field-selection values use the new column
  names; the response table key is `instruction_calls`.
- `transaction_index` is the dense `0..n` rank over stored non-vote
  transactions of the slot, uniform across server ingest sources.

### Added

- Stored columns on `instruction_calls`: `executing_account_index`,
  `account_index_arguments`, `error`, `compute_units_consumed`; on
  `transactions`: `has_dropped_log_messages`.
- Serve-time derived fields: `transaction_id` (`signatures[0]`) on
  transactions and `token_state`
  (`not_a_token` / `opened` / `closed` / `persisted`) on account activity.
- `rollback_guard` on responses (arrow framing carries a guard segment),
  describing the server's in-memory head window; can be absent.
- `InstructionCall::stack_height()` accessor; typed filter values
  throughout the query structs; node typings declare `RowObject`.

### Removed

- **Breaking:** the `include_account_activity` query flag. An empty
  `AccountActivitySelection` (`account_activity: [{}]`) requests every
  activity row in range without forcing blocks into the response.

## [0.2.0-rc.3] - 2026-07-28

Supersedes 0.2.0-rc.2, which shipped a stricter unknown-field policy than we
kept. The only difference is the scope of `deny_unknown_fields`: rc.2 applied
it to the per-selection structs as well, rc.3 scopes it to the query envelope.
Prefer rc.3.

### Added

- `net-types`: `AccountActivitySelection` gains `kind` (`native` / `token`),
  `transaction_id`, and the four position flags (`is_signer`, `is_writable`,
  `is_fee_payer`, `from_lookup_table`).
  - `kind` uses the same predicate the ingest de-merge uses, so a query filter
    and a de-merge always agree on what a native row is. A row carrying both
    sides matches either value, which makes `kind: ["native"]` exactly the row
    set the removed `balances` table held, and `kind: ["token"]` the row set
    `token_balances` held. That is the intended migration for anyone who used
    the split tables to get one side.
  - The flags are `Option<bool>`. A row whose flag is null - the source could
    not derive it - matches neither `true` nor `false`, because unknown is not
    the same as false.
  - Note this filters the response, not the parquet read: there is no
    row-group index on nullness, so a `kind`-filtered query still scans the
    slot range. It removes the rows from the response and from the join key
    set, not the bytes read from disk.

### Changed

- **Breaking:** the query envelope - `SolanaQuery` and `SolanaFieldSelection` -
  now denies unknown fields. A query naming a table or field this version does
  not understand is rejected instead of silently becoming a different query.
  This was motivated by the table removal above: a client still sending
  `balances: [...]` previously deserialized to a query with *no* filters,
  which the server answers with the entire slot range.
  - The per-selection structs deliberately stay lenient, so callers still
    sending the legacy per-selection `include_*` join flags - accepted and
    ignored for several releases - keep working across this upgrade. The
    known cost is that a misspelled filter field inside a selection is
    ignored rather than rejected, which for an AND-ed selection means it
    matches more rows than intended. That boundary is deliberate: the
    envelope catches removed tables, selections stay tolerant.
  - The renames stay wire-compatible - serde aliases are known field names, so
    `instructions`, `program_id` and `field_selection.instruction` still
    deserialize.


## [0.2.0-rc.1] - 2026-07-28

Release candidate. Breaking: the `balances` and `token_balances` tables are
removed and everything they carried is served from `account_activity`.

### Removed

- `schema`: `balance()` and `token_balance()`, their `TABLE_NAMES` entries and
  `schema_for_table` arms. `TABLE_NAMES` is now 6 tables. `table-registry`
  follows automatically.
- `net-types`: `BalanceField`, `TokenBalanceField`, `BalanceSelection`,
  `TokenBalanceSelection`, the `balances` / `token_balances` selection arrays,
  `include_balances` / `include_token_balances`, and `max_num_balances` /
  `max_num_token_balances`.
- `client`: `Balance`, `TokenBalance`, `balances_from_arrow`,
  `token_balances_from_arrow`, the two decode arms and the two
  `QueryResponse` fields.
- `node`: the two selections, their field-selection entries, include flags and
  `maxNum` caps, plus their `index.d.ts` declarations.

### Added

- `net-types`: `AccountActivityField` (18 variants, locked to the parquet
  schema by this crate's coverage test), `AccountActivitySelection`
  (`account` / `mint` / `owner` / `program_id`), `include_account_activity`
  and `max_num_account_activity`.
- `net-types`: `physical_column_name` is now public. Two Wave 2 field renames
  read a column spelled differently from the field (`executing_account` ->
  `program_id`, `account_arguments` -> `accounts`), and that mapping previously
  existed only inside this crate's tests, so a server turning a field selection
  into a column projection had no correct way to do it.
- `client`: `AccountActivity`, `QueryResponse.account_activity`,
  `account_activity_from_arrow` and the `"account_activity"` decode arm. Every
  column but `slot` is read optionally, so a projected response decodes with
  the absent fields `None`.
- `node`: `AccountActivitySelection`, `fieldSelection.accountActivity`,
  `accountActivity`, `includeAccountActivity`, `maxNumAccountActivity`.

### Migration

- `balances: [{account: [A]}]` -> `account_activity: [{account: [A]}]`.
- `token_balances: [{mint: [M]}]` -> `account_activity: [{mint: [M]}]`. A
  non-empty `mint` / `owner` / `program_id` filter selects token rows, since
  native-only rows leave those columns null.
- A single `account_activity` selection expresses what previously needed a
  `balances` selection and a `token_balances` selection joined together.
  Fields within one selection are AND-ed, so "everything for wallet W" is two
  selections: `[{account: [W]}, {owner: [W]}]` - on a native row `account` is
  the wallet, on a token row it is the token account.
- Response field `balances` / `token_balances` -> `account_activity`. Native
  columns (`pre_balance` / `post_balance`) are null on token-only rows and the
  token columns are null on native-only rows; a row where an account had both
  a lamport change and a token movement carries both sides.
- Renamed columns: `pre` / `post` -> `pre_balance` / `post_balance`,
  `pre_amount` / `post_amount` -> `pre_token_balance` / `post_token_balance`.
- New columns with no legacy equivalent: `transaction_id`, `account_index`,
  `is_signer`, `is_writable`, `is_fee_payer`, `from_lookup_table`,
  `token_decimals`.
- Note for servers: `account_activity` carries roughly 4x the rows of
  `balances`, so with the same `max_num_*` cap a range query pages more often.

## [0.1.0] - 2026-07-26

### Added

- `schema`: new `account_activity` table merging native SOL balances and SPL
  token balances into one row per (transaction, account), with reconstructed
  `account_index` and header-derived flags (`is_signer`, `is_writable`,
  `is_fee_payer`, `from_lookup_table`) plus token `mint` / `owner` /
  `token_decimals`. Registered in `TABLE_NAMES` and `schema_for_table`. Coexists
  with the legacy `balances` / `token_balances` tables (no removal). `net-types`
  and `client` are re-released at 0.1.0 unchanged to keep the workspace on a
  single version.

## [0.0.9] - 2026-07-22

### Added

- `net-types`: `SolanaFieldSelection::full_physical()` - a selection naming
  every physical parquet column of every table, in `hypersync-solana-schema`
  order, excluding derived wire fields (`transaction_id`,
  `executing_account_index`, `account_index_arguments`). Intended for
  replication clients (hypersync skar-pull) that must not have columns
  projected away. The variant-to-column mapping is locked to
  `hypersync-solana-schema` by an in-repo test, so every future field must be
  classified as physical or derived for CI to pass.
- `net-types`: all seven `*Field` enums now derive `strum::VariantArray`
  (`InstructionField::VARIANTS` etc.) for exhaustive variant enumeration.

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
