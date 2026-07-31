use crate::*;
use arrow::datatypes::DataType;

#[test]
fn test_all_schemas_have_slot_column() {
    for &name in TABLE_NAMES {
        let schema = schema_for_table(name).unwrap_or_else(|| panic!("schema for {}", name));
        assert!(
            schema.field_with_name("slot").is_ok(),
            "Table {} missing 'slot' column",
            name
        );
    }
}

#[test]
fn test_block_schema_fields() {
    let schema = block();
    assert_eq!(schema.fields().len(), 6);
    assert!(schema.field_with_name("slot").is_ok());
    assert!(schema.field_with_name("blockhash").is_ok());
    assert!(schema.field_with_name("parent_slot").is_ok());
    assert!(schema.field_with_name("parent_blockhash").is_ok());
    assert!(schema.field_with_name("block_time").is_ok());
    assert!(schema.field_with_name("block_height").is_ok());
}

#[test]
fn test_instruction_schema_has_discriminators() {
    let schema = instruction_call();
    assert!(schema.field_with_name("d1").is_ok());
    assert!(schema.field_with_name("d2").is_ok());
    assert!(schema.field_with_name("d4").is_ok());
    assert!(schema.field_with_name("d8").is_ok());
}

#[test]
fn test_instruction_call_wire_aligned_columns() {
    let schema = instruction_call();
    // Physical names match wire names since the Wave 2 API lock.
    for name in ["executing_account", "account_arguments", "tx_success"] {
        assert!(schema.field_with_name(name).is_ok(), "missing {name}");
    }
    for legacy in ["program_id", "accounts", "is_committed"] {
        assert!(
            schema.field_with_name(legacy).is_err(),
            "legacy column {legacy} should be gone"
        );
    }
    // Per-invocation error / compute units (SQD-derived completeness).
    assert_eq!(
        schema.field_with_name("error").unwrap().data_type(),
        &DataType::Utf8
    );
    assert_eq!(
        schema
            .field_with_name("compute_units_consumed")
            .unwrap()
            .data_type(),
        &DataType::UInt64
    );
}

#[test]
fn test_transaction_has_dropped_log_messages() {
    let schema = transaction();
    let f = schema.field_with_name("has_dropped_log_messages").unwrap();
    assert_eq!(f.data_type(), &DataType::Boolean);
    assert!(f.is_nullable());
}

#[test]
fn test_instruction_schema_has_account_positions() {
    let schema = instruction_call();
    for i in 0..10 {
        let name = format!("a{}", i);
        assert!(
            schema.field_with_name(&name).is_ok(),
            "Missing account position column {}",
            name
        );
    }
}

#[test]
fn test_six_tables() {
    assert_eq!(TABLE_NAMES.len(), 6);
    for &name in TABLE_NAMES {
        assert!(
            schema_for_table(name).is_some(),
            "Missing schema for {}",
            name
        );
    }
}

#[test]
fn test_account_activity_columns() {
    let schema = account_activity();
    // Grain + identity.
    assert_eq!(
        schema.field_with_name("slot").unwrap().data_type(),
        &DataType::UInt64
    );
    assert!(!schema.field_with_name("slot").unwrap().is_nullable());
    for name in [
        "transaction_index",
        "transaction_id",
        "account_index",
        "account",
    ] {
        assert!(schema.field_with_name(name).is_ok(), "missing {name}");
    }
    // Native SOL columns.
    assert_eq!(
        schema.field_with_name("pre_balance").unwrap().data_type(),
        &DataType::UInt64
    );
    assert_eq!(
        schema.field_with_name("post_balance").unwrap().data_type(),
        &DataType::UInt64
    );
    // Flags.
    for name in [
        "is_signer",
        "is_writable",
        "is_fee_payer",
        "from_lookup_table",
    ] {
        assert_eq!(
            schema.field_with_name(name).unwrap().data_type(),
            &DataType::Boolean,
            "flag {name} should be Boolean"
        );
    }
    // Token columns. Balances are decimal strings, carried verbatim from the
    // source (raw SPL amounts are u64 on-chain; Utf8 is for verbatim carry,
    // not range).
    for name in ["pre_owner", "post_owner"] {
        assert_eq!(
            schema.field_with_name(name).unwrap().data_type(),
            &DataType::Utf8,
            "{name} should be Utf8"
        );
    }
    assert!(
        schema.field_with_name("owner").is_err(),
        "collapsed owner column should be gone"
    );
    assert_eq!(
        schema
            .field_with_name("token_decimals")
            .unwrap()
            .data_type(),
        &DataType::UInt8
    );
    assert_eq!(
        schema
            .field_with_name("pre_token_balance")
            .unwrap()
            .data_type(),
        &DataType::Utf8
    );
    assert_eq!(
        schema
            .field_with_name("post_token_balance")
            .unwrap()
            .data_type(),
        &DataType::Utf8
    );
    // pre/post program id distinguish SPL Token from Token-2022.
    assert!(schema.field_with_name("pre_program_id").is_ok());
    assert!(schema.field_with_name("post_program_id").is_ok());
}

#[test]
fn test_unknown_table_returns_none() {
    assert!(schema_for_table("nonexistent").is_none());
}
