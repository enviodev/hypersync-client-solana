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
    let schema = instruction();
    assert!(schema.field_with_name("d1").is_ok());
    assert!(schema.field_with_name("d2").is_ok());
    assert!(schema.field_with_name("d4").is_ok());
    assert!(schema.field_with_name("d8").is_ok());
}

#[test]
fn test_instruction_schema_has_account_positions() {
    let schema = instruction();
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
fn test_token_balance_has_program_ids() {
    let schema = token_balance();
    // pre/post program id distinguish SPL Token from Token-2022.
    assert!(schema.field_with_name("pre_program_id").is_ok());
    assert!(schema.field_with_name("post_program_id").is_ok());
    // Amounts are decimal strings so Token-2022 amounts above u64::MAX survive.
    assert_eq!(
        schema.field_with_name("pre_amount").unwrap().data_type(),
        &DataType::Utf8
    );
    assert_eq!(
        schema.field_with_name("post_amount").unwrap().data_type(),
        &DataType::Utf8
    );
}

#[test]
fn test_seven_tables() {
    assert_eq!(TABLE_NAMES.len(), 7);
    for &name in TABLE_NAMES {
        assert!(
            schema_for_table(name).is_some(),
            "Missing schema for {}",
            name
        );
    }
}

#[test]
fn test_unknown_table_returns_none() {
    assert!(schema_for_table("nonexistent").is_none());
}
