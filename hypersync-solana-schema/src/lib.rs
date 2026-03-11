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

pub fn balance() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("transaction_index", DataType::UInt32, true),
        Field::new("account", DataType::Utf8, true),
        Field::new("pre", DataType::UInt64, true),
        Field::new("post", DataType::UInt64, true),
    ]))
}

pub fn token_balance() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("transaction_index", DataType::UInt32, true),
        Field::new("account", DataType::Utf8, true),
        Field::new("mint", DataType::Utf8, true),
        Field::new("owner", DataType::Utf8, true),
        Field::new("pre_amount", DataType::Utf8, true),
        Field::new("post_amount", DataType::Utf8, true),
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
    "balances",
    "token_balances",
    "rewards",
];

/// Returns the Arrow schema for the given Solana table name.
pub fn schema_for_table(table: &str) -> Option<SchemaRef> {
    match table {
        "blocks" => Some(block()),
        "transactions" => Some(transaction()),
        "instructions" => Some(instruction()),
        "logs" => Some(log()),
        "balances" => Some(balance()),
        "token_balances" => Some(token_balance()),
        "rewards" => Some(reward()),
        _ => None,
    }
}
