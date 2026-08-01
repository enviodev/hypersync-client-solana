#[cfg(test)]
mod tests;

use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema, SchemaRef};

pub fn block() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("blockhash", DataType::Utf8, false),
        Field::new("parent_slot", DataType::UInt64, true),
        Field::new("parent_blockhash", DataType::Utf8, true),
        Field::new("block_time", DataType::Int64, true),
        Field::new("block_height", DataType::UInt64, true),
    ]))
}

pub fn transaction() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("transaction_index", DataType::UInt32, false),
        Field::new(
            "signatures",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            false,
        ),
        Field::new("fee_payer", DataType::Utf8, true),
        Field::new("success", DataType::Boolean, true),
        Field::new("err", DataType::Utf8, true),
        Field::new("fee", DataType::UInt64, true),
        Field::new("compute_units_consumed", DataType::UInt64, true),
        Field::new(
            "account_keys",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("recent_blockhash", DataType::Utf8, true),
        Field::new("version", DataType::Utf8, true),
        Field::new(
            "loaded_addresses_writable",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new(
            "loaded_addresses_readonly",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
    ]))
}

pub fn instruction() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("transaction_index", DataType::UInt32, false),
        Field::new(
            "instruction_address",
            DataType::List(Arc::new(Field::new("item", DataType::UInt32, true))),
            false,
        ),
        Field::new("program_id", DataType::Utf8, false),
        Field::new(
            "accounts",
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new("data", DataType::Binary, true),
        Field::new("d1", DataType::FixedSizeBinary(1), true),
        Field::new("d2", DataType::FixedSizeBinary(2), true),
        Field::new("d4", DataType::FixedSizeBinary(4), true),
        Field::new("d8", DataType::FixedSizeBinary(8), true),
        Field::new("a0", DataType::Utf8, true),
        Field::new("a1", DataType::Utf8, true),
        Field::new("a2", DataType::Utf8, true),
        Field::new("a3", DataType::Utf8, true),
        Field::new("a4", DataType::Utf8, true),
        Field::new("a5", DataType::Utf8, true),
        Field::new("a6", DataType::Utf8, true),
        Field::new("a7", DataType::Utf8, true),
        Field::new("a8", DataType::Utf8, true),
        Field::new("a9", DataType::Utf8, true),
        Field::new("is_inner", DataType::Boolean, false),
        Field::new("is_committed", DataType::Boolean, false),
    ]))
}

pub fn log() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("transaction_index", DataType::UInt32, true),
        Field::new(
            "instruction_address",
            DataType::List(Arc::new(Field::new("item", DataType::UInt32, true))),
            true,
        ),
        Field::new("program_id", DataType::Utf8, true),
        Field::new("kind", DataType::Utf8, true),
        Field::new("message", DataType::Utf8, true),
    ]))
}

/// Unified per-(transaction, account) activity table (v1).
///
/// Merges native SOL balance changes (`balances`) and SPL token balance
/// metadata (`token_balances`) into one filterable row per (slot,
/// transaction_index, account), following the changed/touched-only semantic:
/// native columns populated when the account had a lamport change, token
/// columns populated when the account appears in token-balance metadata, null
/// otherwise. Spec: Work/solana/account-activity-spec.md.
pub fn account_activity() -> SchemaRef {
    Arc::new(Schema::new(vec![
        // Identity / grain.
        Field::new("slot", DataType::UInt64, false),
        Field::new("transaction_index", DataType::UInt32, true),
        Field::new("transaction_id", DataType::Utf8, true),
        Field::new("account_index", DataType::UInt32, true),
        Field::new("account", DataType::Utf8, true),
        // Native SOL (null when no native change on this account in this tx).
        //
        // Deliberately NOT shared with pre/post_token_balance below. The native
        // and token sides are independent axes, not two encodings of one value:
        // a single row commonly carries both, since a token account also holds
        // lamports. Wrapped SOL is the clearest case - lamports equal the token
        // amount plus the rent-exempt reserve, so the two differ by a constant
        // and both are needed. Even where they coincide the units differ:
        // lamports are always 1e-9 SOL, whereas a token amount is in raw base
        // units scaled by `token_decimals` on the same row.
        Field::new("pre_balance", DataType::UInt64, true),
        Field::new("post_balance", DataType::UInt64, true),
        // Header-derived flags (null only if not derivable).
        Field::new("is_signer", DataType::Boolean, true),
        Field::new("is_writable", DataType::Boolean, true),
        Field::new("is_fee_payer", DataType::Boolean, true),
        Field::new("from_lookup_table", DataType::Boolean, true),
        // SPL token (null on non-token rows).
        Field::new("mint", DataType::Utf8, true),
        Field::new("owner", DataType::Utf8, true),
        Field::new("token_decimals", DataType::UInt8, true),
        // Raw base units, carried verbatim as the decimal string the source
        // reported, so no parse step can fail or silently coerce. See the note
        // on pre_balance for why this is a separate column, not a reuse of it.
        Field::new("pre_token_balance", DataType::Utf8, true),
        Field::new("post_token_balance", DataType::Utf8, true),
        Field::new("pre_program_id", DataType::Utf8, true),
        Field::new("post_program_id", DataType::Utf8, true),
    ]))
}

pub fn reward() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("pubkey", DataType::Utf8, true),
        Field::new("lamports", DataType::Int64, true),
        Field::new("post_balance", DataType::UInt64, true),
        Field::new("reward_type", DataType::Utf8, true),
        Field::new("commission", DataType::UInt8, true),
    ]))
}

/// All Solana table names in deterministic order.
pub const TABLE_NAMES: &[&str] = &[
    "blocks",
    "transactions",
    "instructions",
    "logs",
    "account_activity",
    "rewards",
];

/// Returns the Arrow schema for the given Solana table name.
pub fn schema_for_table(table: &str) -> Option<SchemaRef> {
    match table {
        "blocks" => Some(block()),
        "transactions" => Some(transaction()),
        "instructions" => Some(instruction()),
        "logs" => Some(log()),
        "account_activity" => Some(account_activity()),
        "rewards" => Some(reward()),
        _ => None,
    }
}
