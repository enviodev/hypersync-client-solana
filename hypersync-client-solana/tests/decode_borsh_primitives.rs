//! Spike: primitive-level round-trip tests for the schema-driven Borsh
//! interpreter.
//!
//! Strategy: encode known Rust values via `borsh::to_vec` (the same
//! serializer Solana programs use), then decode the resulting byte stream
//! through our `FieldType` interpreter and assert the JSON shape matches the
//! locked output convention.

use std::collections::BTreeMap;

use borsh::{BorshSerialize, to_vec};
use hypersync_client_solana::decode::{
    decode_field, decode_top_level, DecodeError, DefinedTypes, EnumVariant, FieldType, NamedField,
};
use serde_json::{json, Value};

fn no_defined() -> DefinedTypes {
    BTreeMap::new()
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

#[test]
fn bool_roundtrip() {
    assert_eq!(
        decode_top_level(&FieldType::Bool, &no_defined(), &to_vec(&true).unwrap()).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        decode_top_level(&FieldType::Bool, &no_defined(), &to_vec(&false).unwrap()).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn invalid_bool_byte_errors() {
    let err = decode_top_level(&FieldType::Bool, &no_defined(), &[2]).unwrap_err();
    assert!(matches!(err, DecodeError::InvalidBool(2)));
}

#[test]
fn sub_64_bit_integers_render_as_json_number() {
    let cases: &[(FieldType, Vec<u8>, Value)] = &[
        (FieldType::U8, to_vec(&7u8).unwrap(), json!(7)),
        (FieldType::U16, to_vec(&65000u16).unwrap(), json!(65000)),
        (FieldType::U32, to_vec(&4_000_000_000u32).unwrap(), json!(4_000_000_000u64)),
        (FieldType::I8, to_vec(&-5i8).unwrap(), json!(-5)),
        (FieldType::I16, to_vec(&-30000i16).unwrap(), json!(-30000)),
        (FieldType::I32, to_vec(&-2_000_000_000i32).unwrap(), json!(-2_000_000_000i64)),
    ];
    for (ty, bytes, expected) in cases {
        let got = decode_top_level(ty, &no_defined(), bytes).unwrap();
        assert_eq!(&got, expected, "{:?}", ty);
        assert!(got.is_number(), "{:?} should be JSON number", ty);
    }
}

#[test]
fn at_or_above_64_bit_integers_render_as_decimal_string() {
    // The locked rule: anything >= 64 bits stringifies (decimal), even when
    // the actual value would fit in JSON's 2^53 number range. Symmetric, so
    // downstream TypeScript can pick its `bigint` boundary at the type level.
    let cases: &[(FieldType, Vec<u8>, &str)] = &[
        (FieldType::U64, to_vec(&42u64).unwrap(), "42"),
        (FieldType::U64, to_vec(&u64::MAX).unwrap(), "18446744073709551615"),
        (FieldType::U128, to_vec(&u128::MAX).unwrap(), "340282366920938463463374607431768211455"),
        (FieldType::I64, to_vec(&-42i64).unwrap(), "-42"),
        (FieldType::I64, to_vec(&i64::MIN).unwrap(), "-9223372036854775808"),
        (FieldType::I128, to_vec(&i128::MIN).unwrap(), "-170141183460469231731687303715884105728"),
    ];
    for (ty, bytes, expected_str) in cases {
        let got = decode_top_level(ty, &no_defined(), bytes).unwrap();
        assert_eq!(got, Value::String(expected_str.to_string()), "{:?}", ty);
    }
}

#[test]
fn floats_roundtrip_and_handle_finite() {
    let f32_bytes = to_vec(&-1.5f32).unwrap();
    let f32_got = decode_top_level(&FieldType::F32, &no_defined(), &f32_bytes).unwrap();
    assert_eq!(f32_got.as_f64().unwrap() as f32, -1.5f32);

    let f64_bytes = to_vec(&123.456f64).unwrap();
    let f64_got = decode_top_level(&FieldType::F64, &no_defined(), &f64_bytes).unwrap();
    assert!((f64_got.as_f64().unwrap() - 123.456f64).abs() < 1e-12);
}

#[test]
fn string_roundtrip() {
    let bytes = to_vec(&"hello, hyperindex".to_string()).unwrap();
    let got = decode_top_level(&FieldType::String, &no_defined(), &bytes).unwrap();
    assert_eq!(got, json!("hello, hyperindex"));
}

#[test]
fn empty_string_roundtrip() {
    let bytes = to_vec(&"".to_string()).unwrap();
    let got = decode_top_level(&FieldType::String, &no_defined(), &bytes).unwrap();
    assert_eq!(got, json!(""));
}

#[test]
fn bytes_render_as_hex_string() {
    let payload: Vec<u8> = vec![0x00, 0x10, 0xff, 0xab];
    let bytes = to_vec(&payload).unwrap();
    let got = decode_top_level(&FieldType::Bytes, &no_defined(), &bytes).unwrap();
    assert_eq!(got, json!("0x0010ffab"));
}

#[test]
fn pubkey_renders_as_base58() {
    // The all-1s pubkey is the canonical "fake" key Solana docs use.
    let bytes: [u8; 32] = [1u8; 32];
    let mut encoded = Vec::with_capacity(32);
    bytes.serialize(&mut encoded).unwrap();
    let got = decode_top_level(&FieldType::Pubkey, &no_defined(), &encoded).unwrap();
    // Round-trip via bs58 to validate the encoding matches our interpreter's
    // expectation, rather than hard-coding the (correct) string literal.
    let expected_b58 = bs58::encode(bytes).into_string();
    assert_eq!(got, Value::String(expected_b58));
}

// ---------------------------------------------------------------------------
// Composites
// ---------------------------------------------------------------------------

#[test]
fn option_some_and_none() {
    let some_bytes = to_vec(&Some(7u32)).unwrap();
    let some_got = decode_top_level(
        &FieldType::Option(Box::new(FieldType::U32)),
        &no_defined(),
        &some_bytes,
    )
    .unwrap();
    assert_eq!(some_got, json!(7));

    let none_bytes = to_vec(&Option::<u32>::None).unwrap();
    let none_got = decode_top_level(
        &FieldType::Option(Box::new(FieldType::U32)),
        &no_defined(),
        &none_bytes,
    )
    .unwrap();
    assert_eq!(none_got, Value::Null);
}

#[test]
fn vec_of_primitives_and_empty_vec() {
    let payload = vec![1u32, 2, 3, 4];
    let bytes = to_vec(&payload).unwrap();
    let got = decode_top_level(&FieldType::Vec(Box::new(FieldType::U32)), &no_defined(), &bytes)
        .unwrap();
    assert_eq!(got, json!([1, 2, 3, 4]));

    let empty: Vec<u32> = vec![];
    let bytes = to_vec(&empty).unwrap();
    let got = decode_top_level(&FieldType::Vec(Box::new(FieldType::U32)), &no_defined(), &bytes)
        .unwrap();
    assert_eq!(got, json!([]));
}

#[test]
fn array_of_u8_len_32_renders_as_base58_pubkey() {
    // Verifies the locked 32-byte = pubkey convention also applies to raw
    // `[u8; 32]` array fields, not just `FieldType::Pubkey`. This is what
    // makes Anchor IDL `{ "array": ["u8", 32] }` decode the same as `"pubkey"`.
    let bytes: [u8; 32] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ];
    let mut encoded = Vec::with_capacity(32);
    bytes.serialize(&mut encoded).unwrap();
    let ty = FieldType::Array {
        ty: Box::new(FieldType::U8),
        len: 32,
    };
    let got = decode_top_level(&ty, &no_defined(), &encoded).unwrap();
    assert_eq!(got, Value::String(bs58::encode(bytes).into_string()));
}

#[test]
fn array_of_other_size_stays_as_json_array() {
    // Non-32-byte u8 arrays do *not* trigger the pubkey special case.
    let payload: [u8; 4] = [10, 20, 30, 40];
    let mut encoded = Vec::with_capacity(4);
    payload.serialize(&mut encoded).unwrap();
    let ty = FieldType::Array {
        ty: Box::new(FieldType::U8),
        len: 4,
    };
    let got = decode_top_level(&ty, &no_defined(), &encoded).unwrap();
    assert_eq!(got, json!([10, 20, 30, 40]));
}

#[test]
fn struct_roundtrip() {
    #[derive(BorshSerialize)]
    struct Sample {
        a: u32,
        b: String,
        c: bool,
    }
    let sample = Sample { a: 7, b: "hi".to_string(), c: true };
    let bytes = to_vec(&sample).unwrap();
    let ty = FieldType::Struct(vec![
        NamedField { name: "a".into(), ty: FieldType::U32 },
        NamedField { name: "b".into(), ty: FieldType::String },
        NamedField { name: "c".into(), ty: FieldType::Bool },
    ]);
    let got = decode_top_level(&ty, &no_defined(), &bytes).unwrap();
    assert_eq!(got, json!({ "a": 7, "b": "hi", "c": true }));
}

#[test]
fn nested_struct_roundtrip() {
    #[derive(BorshSerialize)]
    struct Inner {
        amount: u64,
    }
    #[derive(BorshSerialize)]
    struct Outer {
        label: String,
        inner: Inner,
    }
    let outer = Outer {
        label: "stake".to_string(),
        inner: Inner { amount: 500_000 },
    };
    let bytes = to_vec(&outer).unwrap();
    let ty = FieldType::Struct(vec![
        NamedField { name: "label".into(), ty: FieldType::String },
        NamedField {
            name: "inner".into(),
            ty: FieldType::Struct(vec![
                NamedField { name: "amount".into(), ty: FieldType::U64 },
            ]),
        },
    ]);
    let got = decode_top_level(&ty, &no_defined(), &bytes).unwrap();
    // amount is u64 so it stringifies.
    assert_eq!(got, json!({ "label": "stake", "inner": { "amount": "500000" } }));
}

#[test]
fn enum_unit_variant() {
    // Three-variant enum, the second one selected.
    #[derive(BorshSerialize)]
    enum E { A, B, C }
    let bytes = to_vec(&E::B).unwrap();
    let ty = FieldType::Enum(vec![
        EnumVariant { name: "A".into(), fields: None },
        EnumVariant { name: "B".into(), fields: None },
        EnumVariant { name: "C".into(), fields: None },
    ]);
    let got = decode_top_level(&ty, &no_defined(), &bytes).unwrap();
    assert_eq!(got, json!({ "B": {} }));
}

#[test]
fn enum_struct_variant() {
    #[derive(BorshSerialize)]
    enum E {
        Idle,
        Active { user: u32, score: u64 },
    }
    let bytes = to_vec(&E::Active { user: 42, score: 9_999_999_999_u64 }).unwrap();
    let ty = FieldType::Enum(vec![
        EnumVariant { name: "Idle".into(), fields: None },
        EnumVariant {
            name: "Active".into(),
            fields: Some(vec![
                NamedField { name: "user".into(), ty: FieldType::U32 },
                NamedField { name: "score".into(), ty: FieldType::U64 },
            ]),
        },
    ]);
    let got = decode_top_level(&ty, &no_defined(), &bytes).unwrap();
    assert_eq!(got, json!({ "Active": { "user": 42, "score": "9999999999" } }));
}

#[test]
fn unknown_enum_variant_index_errors() {
    // Three variants declared, byte stream picks index 7.
    let ty = FieldType::Enum(vec![
        EnumVariant { name: "A".into(), fields: None },
        EnumVariant { name: "B".into(), fields: None },
    ]);
    let err = decode_top_level(&ty, &no_defined(), &[7]).unwrap_err();
    assert!(matches!(err, DecodeError::UnknownEnumVariant(7)));
}

#[test]
fn defined_type_resolves_via_registry() {
    // Schema references a named type "AmountStruct" defined elsewhere in the
    // IDL's `types` registry. Decoder follows the indirection.
    #[derive(BorshSerialize)]
    struct AmountStruct {
        value: u64,
    }
    let payload = AmountStruct { value: 123 };
    let bytes = to_vec(&payload).unwrap();

    let mut defined: DefinedTypes = BTreeMap::new();
    defined.insert(
        "AmountStruct".to_string(),
        FieldType::Struct(vec![NamedField { name: "value".into(), ty: FieldType::U64 }]),
    );

    let ty = FieldType::Defined("AmountStruct".to_string());
    let got = decode_top_level(&ty, &defined, &bytes).unwrap();
    assert_eq!(got, json!({ "value": "123" }));
}

#[test]
fn defined_type_missing_errors() {
    let defined: DefinedTypes = BTreeMap::new();
    let ty = FieldType::Defined("NotPresent".to_string());
    let err = decode_field(&ty, &defined, &mut (&[] as &[u8])).unwrap_err();
    match err {
        DecodeError::UnresolvedType(name) => assert_eq!(name, "NotPresent"),
        other => panic!("wrong error: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Safety: trailing bytes + underflow
// ---------------------------------------------------------------------------

#[test]
fn trailing_bytes_rejected_by_top_level() {
    // Encode a u32, append one stray byte, expect TrailingBytes(1).
    let mut bytes = to_vec(&7u32).unwrap();
    bytes.push(0xaa);
    let err = decode_top_level(&FieldType::U32, &no_defined(), &bytes).unwrap_err();
    assert!(matches!(err, DecodeError::TrailingBytes(1)));
}

#[test]
fn buffer_underflow_returns_clear_error() {
    // Schema says U64 (8 bytes), buffer only has 3.
    let err = decode_top_level(&FieldType::U64, &no_defined(), &[0u8, 0, 0]).unwrap_err();
    match err {
        DecodeError::Underflow { what, need, have } => {
            assert_eq!(what, "u64");
            assert_eq!(need, 8);
            assert_eq!(have, 3);
        }
        other => panic!("wrong error: {:?}", other),
    }
}

#[test]
fn underflow_inside_composite() {
    // A Vec<u32> declaring length 4 but providing only 2 elements' worth of
    // bytes (8 of the needed 16). Underflow should bubble up with the inner
    // field's context.
    let mut bytes = (4u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&2u32.to_le_bytes());
    // ... but we omit elements 3 and 4.
    let err = decode_top_level(
        &FieldType::Vec(Box::new(FieldType::U32)),
        &no_defined(),
        &bytes,
    )
    .unwrap_err();
    assert!(matches!(err, DecodeError::Underflow { what: "u32", .. }));
}
