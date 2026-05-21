//! End-to-end coverage for the Anchor IDL parser + decoder dispatch.
//!
//! Two fixture IDLs (modern 0.30+ and legacy 0.29) live inline so the test
//! is self-contained. We then build a synthetic `Instruction` whose `data`
//! field is `discriminator || borsh(args)` and pipe it through
//! `decode_instruction`.

use borsh::BorshSerialize;
use hypersync_client_solana::decode::{
    anchor_idl::legacy_discriminator, decode_instruction, schema_from_anchor_idl_json, FieldType,
    ProgramSchema,
};
use hypersync_client_solana::simple_types::Instruction;
use serde_json::json;

/// Build the bare-minimum `Instruction` the decoder needs. Fields the
/// decoder ignores (slot, transaction_index, etc.) are left default.
fn instr_from(program_id: &str, data: Vec<u8>, accounts: Vec<&str>) -> Instruction {
    Instruction {
        program_id: program_id.to_string(),
        accounts: accounts.into_iter().map(str::to_string).collect(),
        data,
        ..Default::default()
    }
}

const MODERN_IDL: &str = r#"{
  "address": "M0dErNxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [
    {
      "name": "create_thing",
      "discriminator": [1, 2, 3, 4, 5, 6, 7, 8],
      "accounts": [
        { "name": "thing", "writable": true, "signer": false },
        { "name": "payer", "writable": true, "signer": true }
      ],
      "args": [
        { "name": "kind", "type": { "defined": { "name": "Kind" } } },
        { "name": "count", "type": "u32" }
      ]
    }
  ],
  "types": [
    {
      "name": "Kind",
      "type": {
        "kind": "enum",
        "variants": [
          { "name": "Alpha" },
          { "name": "Beta", "fields": [ { "name": "size", "type": "u64" } ] }
        ]
      }
    }
  ]
}"#;

#[test]
fn modern_idl_parses_and_decodes() {
    let schema = schema_from_anchor_idl_json(MODERN_IDL).expect("parse modern IDL");
    assert_eq!(
        schema.program_id,
        "M0dErNxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
    );
    assert_eq!(schema.disc_lens, vec![8]);
    assert_eq!(schema.instructions.len(), 1);

    // Spot-check defined types.
    let kind = schema
        .defined_types
        .get("Kind")
        .expect("Kind in defined_types");
    let FieldType::Enum(variants) = kind else {
        panic!("Kind should be enum, got {:?}", kind)
    };
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].name, "Alpha");
    assert!(variants[0].fields.is_none(), "Alpha is unit");
    assert_eq!(variants[1].name, "Beta");

    // Build a payload: disc || enum_variant(1) || borsh(size=42u64) || count=7u32.
    let mut data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    data.push(1); // Beta variant index
    data.extend_from_slice(&42u64.to_le_bytes()); // size
    data.extend_from_slice(&7u32.to_le_bytes()); // count

    let ix = instr_from(
        &schema.program_id,
        data,
        vec!["AccThing", "AccPayer", "AccExtra"],
    );
    let decoded = decode_instruction(&schema, &ix).expect("decode");
    assert_eq!(decoded.name, "create_thing");
    assert_eq!(
        decoded.args,
        json!({
            "kind": { "Beta": { "size": "42" } },
            "count": 7
        })
    );
    assert_eq!(
        decoded.named_accounts.get("thing").map(|s| s.as_str()),
        Some("AccThing")
    );
    assert_eq!(
        decoded.named_accounts.get("payer").map(|s| s.as_str()),
        Some("AccPayer")
    );
    assert_eq!(decoded.extra_accounts, vec!["AccExtra"]);
}

const LEGACY_IDL: &str = r#"{
  "version": "0.29.0",
  "name": "demo",
  "instructions": [
    {
      "name": "initialize",
      "accounts": [
        { "name": "state", "isMut": true, "isSigner": false },
        { "name": "signer", "isMut": false, "isSigner": true }
      ],
      "args": [
        { "name": "a", "type": "u64" },
        { "name": "b", "type": "bool" },
        { "name": "key", "type": "publicKey" }
      ]
    }
  ]
}"#;

#[test]
fn legacy_idl_computes_discriminator_from_name() {
    let schema = schema_from_anchor_idl_json(LEGACY_IDL).expect("parse legacy IDL");
    assert_eq!(schema.disc_lens, vec![8]);

    // Independently compute what the parser should have used.
    let expected_disc = legacy_discriminator("initialize");
    assert_eq!(expected_disc.len(), 8);
    assert!(
        schema.instructions.contains_key(&expected_disc),
        "legacy disc not found in schema; got keys: {:?}",
        schema.instructions.keys().collect::<Vec<_>>()
    );

    // Round-trip a payload.
    let pubkey_bytes: [u8; 32] = [
        0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff,
    ];
    let mut data = expected_disc.clone();
    data.extend_from_slice(&123u64.to_le_bytes()); // a
    data.push(1); // b = true
    data.extend_from_slice(&pubkey_bytes); // key

    let ix = instr_from("LegacyProgId", data, vec!["state_pk", "signer_pk"]);
    let decoded = decode_instruction(&schema, &ix).expect("decode");
    assert_eq!(decoded.name, "initialize");
    assert_eq!(
        decoded.args,
        json!({
            "a": "123",
            "b": true,
            "key": bs58::encode(pubkey_bytes).into_string(),
        })
    );
    assert!(decoded.extra_accounts.is_empty());
}

#[test]
fn legacy_discriminator_matches_anchor_recipe() {
    // Anchor's recipe: sha256("global:<snake_case_name>")[..8].
    // We computed the same in the parser; this asserts the helper is
    // accessible and stable across versions.
    let d = legacy_discriminator("initialize");
    // Compare against the explicit byte sequence so a future change to the
    // helper would surface clearly.
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"global:initialize");
    let expected: Vec<u8> = h.finalize()[..8].to_vec();
    assert_eq!(d, expected);
}

#[test]
fn legacy_snake_case_conversion() {
    // Legacy IDLs typically express instruction names in camelCase, but
    // Anchor computes the disc from the underlying Rust function name
    // (snake_case). Our parser does the conversion before hashing.
    let legacy_camel = r#"{
      "version": "0.29.0",
      "name": "demo",
      "instructions": [
        { "name": "createThingV2", "accounts": [], "args": [] }
      ]
    }"#;
    let schema = schema_from_anchor_idl_json(legacy_camel).expect("parse");
    let expected = legacy_discriminator("createThingV2"); // helper handles snake-case internally
    assert!(schema.instructions.contains_key(&expected));

    // Independently verify the snake-cased name is what's hashed.
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"global:create_thing_v2");
    let direct: Vec<u8> = h.finalize()[..8].to_vec();
    assert_eq!(expected, direct);
}

#[test]
fn unknown_discriminator_surfaces_full_probe_window() {
    let schema = schema_from_anchor_idl_json(MODERN_IDL).expect("parse");
    let bogus_data = vec![0xaa; 16];
    let ix = instr_from(&schema.program_id, bogus_data, vec!["a", "b"]);
    let err = decode_instruction(&schema, &ix).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("unknown discriminator"), "got: {msg}");
    // Should display the 8-byte probe (longest disc_len).
    assert!(msg.contains("aaaaaaaaaaaaaaaa"), "got: {msg}");
}

#[test]
fn too_few_accounts_errors() {
    let schema: ProgramSchema = schema_from_anchor_idl_json(MODERN_IDL).expect("parse");
    let mut data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    data.push(0); // Alpha (unit)
    data.extend_from_slice(&0u32.to_le_bytes());
    let ix = instr_from(&schema.program_id, data, vec!["only_one"]);
    let err = decode_instruction(&schema, &ix).unwrap_err();
    assert!(
        format!("{err}").contains("expects at least 2"),
        "got: {err}"
    );
}

#[test]
fn modern_idl_type_alias_resolves() {
    // Anchor's IDL spec includes a `kind: "type"` variant for type aliases
    // (e.g. `pub type Lamports = u64`). Ensure the parser resolves the alias
    // body transparently rather than rejecting the type entry.
    let idl_with_alias = r#"{
      "address": "AliasProgxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
      "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
      "instructions": [
        {
          "name": "noop",
          "discriminator": [9, 9, 9, 9, 9, 9, 9, 9],
          "accounts": [],
          "args": [
            { "name": "amount", "type": { "defined": { "name": "Lamports" } } }
          ]
        }
      ],
      "types": [
        {
          "name": "Lamports",
          "type": { "kind": "type", "alias": "u64" }
        }
      ]
    }"#;

    let schema = schema_from_anchor_idl_json(idl_with_alias).expect("parse alias IDL");
    let resolved = schema
        .defined_types
        .get("Lamports")
        .expect("alias registered");
    assert!(matches!(resolved, FieldType::U64), "got {:?}", resolved);

    // Round-trip: u64 args serialize as decimal string per locked conventions.
    let mut data: Vec<u8> = vec![9, 9, 9, 9, 9, 9, 9, 9];
    data.extend_from_slice(&500u64.to_le_bytes());
    let ix = instr_from(&schema.program_id, data, vec![]);
    let decoded = decode_instruction(&schema, &ix).expect("decode alias");
    assert_eq!(decoded.args, json!({ "amount": "500" }));
}

#[test]
fn anchor_roundtrip_via_borsh_derive() {
    // Confirms our hand-written schema matches what borsh-derive emits.
    #[derive(BorshSerialize)]
    struct Args {
        kind: KindWire,
        count: u32,
    }
    #[derive(BorshSerialize)]
    #[allow(dead_code)]
    enum KindWire {
        Alpha,
        Beta { size: u64 },
    }

    let schema = schema_from_anchor_idl_json(MODERN_IDL).expect("parse");
    let args = Args {
        kind: KindWire::Beta { size: u64::MAX },
        count: 1,
    };
    let mut data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    data.extend(borsh::to_vec(&args).expect("encode"));
    let ix = instr_from(&schema.program_id, data, vec!["x", "y"]);
    let decoded = decode_instruction(&schema, &ix).expect("decode");
    assert_eq!(
        decoded.args,
        json!({
            "kind": { "Beta": { "size": "18446744073709551615" } },
            "count": 1
        })
    );
}
