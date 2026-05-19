//! Typed Rust structs for Solana HyperSync responses.
//!
//! These mirror the Arrow column layout in `hypersync_solana_schema`. Users who
//! do not want to deal with Arrow directly can call `Client::collect` and get
//! `Vec<T>` for each table.

use serde::{Deserialize, Serialize};

/// Block-level record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub slot: u64,
    pub blockhash: String,
    pub parent_slot: Option<u64>,
    pub parent_blockhash: Option<String>,
    pub block_time: Option<i64>,
    pub block_height: Option<u64>,
}

/// Transaction-level record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub slot: u64,
    pub transaction_index: u32,
    pub signatures: Vec<String>,
    pub fee_payer: Option<String>,
    pub success: Option<bool>,
    pub err: Option<String>,
    pub fee: Option<u64>,
    pub compute_units_consumed: Option<u64>,
    pub account_keys: Vec<String>,
    pub recent_blockhash: Option<String>,
    pub version: Option<String>,
    pub loaded_addresses_writable: Vec<String>,
    pub loaded_addresses_readonly: Vec<String>,
}

/// Instruction-level record.
///
/// `data` is the raw instruction byte buffer. `d1`..`d8` are leading-byte prefix
/// views (the server pre-computes them so filters can be pushed down); they
/// only contain bytes when the underlying instruction is at least that long.
/// `a0`..`a9` are positional account shortcuts that mirror `accounts[0..10]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction {
    pub slot: u64,
    pub transaction_index: u32,
    pub instruction_address: Vec<u32>,
    pub program_id: String,
    pub accounts: Vec<String>,
    pub data: Vec<u8>,
    pub d1: Option<Vec<u8>>,
    pub d2: Option<Vec<u8>>,
    pub d4: Option<Vec<u8>>,
    pub d8: Option<Vec<u8>>,
    pub a0: Option<String>,
    pub a1: Option<String>,
    pub a2: Option<String>,
    pub a3: Option<String>,
    pub a4: Option<String>,
    pub a5: Option<String>,
    pub a6: Option<String>,
    pub a7: Option<String>,
    pub a8: Option<String>,
    pub a9: Option<String>,
    pub is_inner: bool,
    pub is_committed: bool,
}

/// Log-level record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Log {
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub instruction_address: Option<Vec<u32>>,
    pub program_id: Option<String>,
    pub kind: Option<String>,
    pub message: Option<String>,
}

/// SOL balance change for one account in one transaction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub account: Option<String>,
    pub pre: Option<u64>,
    pub post: Option<u64>,
}

/// SPL token balance change for one account in one transaction.
///
/// `pre_amount` and `post_amount` are kept as strings to preserve the full u64
/// precision the server sends without surprising the user with parsing failures.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenBalance {
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub account: Option<String>,
    pub mint: Option<String>,
    pub owner: Option<String>,
    pub pre_amount: Option<String>,
    pub post_amount: Option<String>,
}

/// Validator / staking reward.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reward {
    pub slot: u64,
    pub pubkey: Option<String>,
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
    pub instructions: Vec<Instruction>,
    pub logs: Vec<Log>,
    pub balances: Vec<Balance>,
    pub token_balances: Vec<TokenBalance>,
    pub rewards: Vec<Reward>,
    /// Raw response size in bytes (sum across all chunks, when used via `collect`).
    pub response_bytes: usize,
}
