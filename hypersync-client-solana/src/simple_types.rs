//! Typed Rust structs for Solana HyperSync responses.
//!
//! These mirror the Arrow column layout in `hypersync_solana_schema`. Users who
//! do not want to deal with Arrow directly can call `Client::collect` and get
//! `Vec<T>` for each table.
//!
//! Every field is `Option<T>`: with `field_selection` any column can be
//! projected away, and `None` means exactly "not selected or the source could
//! not supply it". Identity columns (`slot`, `transaction_index`) are in
//! practice always selected, so unwrapping them is safe in queries that select
//! them; the types do not pretend otherwise.
//!
//! Pubkeys, hashes, and signatures are the byte newtypes from
//! `hypersync_solana_net_types` ([`Address`], [`Hash`], [`Signature`]); base58
//! is presentation-only, so the JSON serde form of these structs is unchanged
//! from the plain-string era.

use serde::{Deserialize, Serialize};

pub use hypersync_solana_net_types::{
    Address, Hash, LogKind, RollbackGuard, Signature, TokenState,
};

/// Serde for `Option<u64>` carried as a JSON string (raw token amounts
/// routinely exceed 2^53, so a bare JSON number would lose precision in JS).
mod opt_u64_as_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<u64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(n) => s.serialize_some(&n.to_string()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
        let s: Option<String> = Option::deserialize(d)?;
        s.map(|s| s.parse().map_err(serde::de::Error::custom))
            .transpose()
    }
}

/// Block-level record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub slot: Option<u64>,
    pub blockhash: Option<Hash>,
    pub parent_slot: Option<u64>,
    pub parent_blockhash: Option<Hash>,
    pub block_time: Option<i64>,
    pub block_height: Option<u64>,
}

/// Transaction-level record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub slot: Option<u64>,
    pub transaction_index: Option<u32>,
    /// `signatures[0]` (base58): the canonical Solana transaction id. Derived
    /// at serving time; select `transaction_id` in `field_selection` to get it.
    pub transaction_id: Option<Signature>,
    pub signatures: Option<Vec<Signature>>,
    pub fee_payer: Option<Address>,
    /// `Some(true)` for a committed transaction, `Some(false)` for a failed
    /// one. `None` only when the `success` column was not requested in
    /// `field_selection` or the source could not determine commit status.
    /// Failed transactions are returned by default (HyperSync does not drop
    /// them); treat `None` as "unknown", never as success. To exclude failed
    /// txs, filter with `tx_success: true` (instruction_calls) or
    /// `success: true` (transactions).
    pub success: Option<bool>,
    pub err: Option<String>,
    pub fee: Option<u64>,
    pub compute_units_consumed: Option<u64>,
    pub account_keys: Option<Vec<Address>>,
    pub recent_blockhash: Option<Hash>,
    pub version: Option<String>,
    pub loaded_addresses_writable: Option<Vec<Address>>,
    pub loaded_addresses_readonly: Option<Vec<Address>>,
    /// True when the validator truncated this transaction's log output.
    /// SQD serves the flag directly; RPC/Firehose/Yellowstone ranges derive
    /// it from the "Log truncated" sentinel. `None` = not selected or the
    /// source could not say.
    pub has_dropped_log_messages: Option<bool>,
}

/// One runtime program invocation (outer or inner). Renamed from
/// `Instruction` in the Wave 2 API lock.
///
/// `data` is the raw instruction byte buffer. `d1`..`d8` are leading-byte prefix
/// views (the server pre-computes them so filters can be pushed down); they
/// only contain bytes when the underlying instruction is at least that long.
/// `a0`..`a9` are positional account shortcuts that mirror
/// `account_arguments[0..10]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionCall {
    pub slot: Option<u64>,
    pub transaction_index: Option<u32>,
    /// Full path of this invocation in the transaction's CPI tree: the outer
    /// instruction index, then each nested inner position. Depth is
    /// `len()` (equal to Solana's native `stack_height`; top-level = 1), the
    /// parent is `instruction_address[..len-1]`, and sibling order is the
    /// last element, so the whole tree is reconstructable. See
    /// [`InstructionCall::stack_height`].
    pub instruction_address: Option<Vec<u32>>,
    /// The invoked program's account. Renamed from `program_id`.
    pub executing_account: Option<Address>,
    /// Index of the executing account within the transaction's resolved key
    /// list. Stored at ingest.
    pub executing_account_index: Option<u32>,
    /// The instruction's account arguments (pubkeys). Renamed from `accounts`.
    pub account_arguments: Option<Vec<Address>>,
    /// Indexes of the account arguments within the transaction's resolved key
    /// list. Stored at ingest.
    pub account_index_arguments: Option<Vec<u32>>,
    pub data: Option<Vec<u8>>,
    pub d1: Option<Vec<u8>>,
    pub d2: Option<Vec<u8>>,
    pub d4: Option<Vec<u8>>,
    pub d8: Option<Vec<u8>>,
    pub a0: Option<Address>,
    pub a1: Option<Address>,
    pub a2: Option<Address>,
    pub a3: Option<Address>,
    pub a4: Option<Address>,
    pub a5: Option<Address>,
    pub a6: Option<Address>,
    pub a7: Option<Address>,
    pub a8: Option<Address>,
    pub a9: Option<Address>,
    pub is_inner: Option<bool>,
    /// True when this instruction's PARENT transaction succeeded. Renamed
    /// from `is_committed`. It says nothing about the individual instruction:
    /// Solana metadata only records instructions that actually executed, so a
    /// failed transaction's rows are precisely the instructions that ran
    /// before the failure point, and every one of them has
    /// `tx_success = Some(false)`. Consumers that count effects must filter
    /// or check `tx_success`, because instructions of failed transactions had
    /// their state changes rolled back.
    pub tx_success: Option<bool>,
    /// Per-invocation failure reason (e.g. "custom program error: 0x1").
    pub error: Option<String>,
    /// Per-invocation compute units, when the source recorded them.
    pub compute_units_consumed: Option<u64>,
}

impl InstructionCall {
    /// Depth of this invocation in the CPI call stack, equal to Solana's
    /// native `stack_height` (top-level = 1). Purely a view over
    /// `instruction_address`, which carries strictly more information
    /// (depth AND parentage AND sibling order).
    pub fn stack_height(&self) -> Option<u32> {
        self.instruction_address.as_ref().map(|a| a.len() as u32)
    }
}

/// Log-level record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Log {
    pub slot: Option<u64>,
    pub transaction_index: Option<u32>,
    pub instruction_address: Option<Vec<u32>>,
    pub program_id: Option<Address>,
    /// Kind of the log line. Unrecognized stored values decode as
    /// [`LogKind::Other`] so a future server-side kind cannot break old
    /// clients. See [`LogKind`] for the per-source caveats (SQD and default
    /// RPC ranges only carry `log` / `data` / `other`).
    pub kind: Option<LogKind>,
    pub message: Option<String>,
}

/// One account's activity in one transaction: the native SOL change, the SPL
/// token balance, or both.
///
/// This is the merged view that replaces `Balance` + `TokenBalance`. Native
/// fields (`pre_balance` / `post_balance`) are `None` on a token-only row and
/// the token fields are `None` on a native-only row; a row where the account
/// had both a lamport change and a token movement carries both sides. Select
/// `token_state` to distinguish "token fields not selected" from "not a token
/// account" authoritatively.
///
/// `account_index` is the account's position in the transaction's resolved key
/// list (accountKeys ++ ALT writable ++ ALT readonly); the position flags are
/// derived from the message header and are `None` where a source could not
/// supply them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountActivity {
    pub slot: Option<u64>,
    pub transaction_index: Option<u32>,
    pub transaction_id: Option<Signature>,
    pub account_index: Option<u32>,
    pub account: Option<Address>,
    pub pre_balance: Option<u64>,
    pub post_balance: Option<u64>,
    pub is_signer: Option<bool>,
    pub is_writable: Option<bool>,
    pub is_fee_payer: Option<bool>,
    pub from_lookup_table: Option<bool>,
    pub mint: Option<Address>,
    /// Token-account owner before the tx (`None` if the account was opened
    /// during the tx). Split from the old collapsed `owner` so an
    /// in-transaction SetAuthority(AccountOwner) change is visible.
    pub pre_owner: Option<Address>,
    /// Token-account owner after the tx (`None` if closed during the tx).
    pub post_owner: Option<Address>,
    pub token_decimals: Option<u8>,
    /// Raw token amount before the tx, in base units scaled by
    /// `token_decimals`. Raw SPL amounts are u64 on-chain (in both Token and
    /// Token-2022; only derived UI amounts can exceed u64), so this is a real
    /// integer; the JSON wire carries it as a string to protect JS consumers
    /// from 2^53 precision loss.
    #[serde(default, with = "opt_u64_as_string")]
    pub pre_token_balance: Option<u64>,
    /// Raw token amount after the tx. See `pre_token_balance`.
    #[serde(default, with = "opt_u64_as_string")]
    pub post_token_balance: Option<u64>,
    pub pre_program_id: Option<Address>,
    pub post_program_id: Option<Address>,
    /// Authoritative token-side state of the row, derived at serving time.
    /// `None` only when not selected.
    pub token_state: Option<TokenState>,
}

/// Validator / staking reward.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reward {
    pub slot: Option<u64>,
    pub pubkey: Option<Address>,
    pub lamports: Option<i64>,
    pub post_balance: Option<u64>,
    pub reward_type: Option<String>,
    pub commission: Option<u8>,
}

/// Typed Solana HyperSync response, mirroring `ArrowResponseData` table-for-table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolanaResponse {
    /// The next slot to query from (for pagination).
    pub next_slot: u64,
    pub blocks: Vec<Block>,
    pub transactions: Vec<Transaction>,
    /// Renamed from `instructions`: one row is one runtime program invocation.
    pub instruction_calls: Vec<InstructionCall>,
    pub logs: Vec<Log>,
    pub account_activity: Vec<AccountActivity>,
    pub rewards: Vec<Reward>,
    /// Reorg guard for the scanned range, when the server produced one. On a
    /// paginated `collect`, this is the guard of the LAST page.
    pub rollback_guard: Option<RollbackGuard>,
    /// Raw response size in bytes (sum across all chunks, when used via `collect`).
    pub response_bytes: usize,
}
