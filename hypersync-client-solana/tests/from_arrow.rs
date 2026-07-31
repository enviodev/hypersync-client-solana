//! Round-trip tests: build synthetic Arrow `RecordBatch`es matching the
//! `hypersync_solana_schema` shapes, decode them with `from_arrow`, assert the
//! resulting structs match the inputs.

use std::sync::Arc;

use arrow::array::{
    BinaryArray, BooleanArray, FixedSizeBinaryArray, Int64Array, ListArray, StringArray,
    UInt32Array, UInt64Array, UInt8Array,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};
use arrow::record_batch::RecordBatch;

use hypersync_client_solana::from_arrow::*;
use hypersync_client_solana::simple_types::{Address, Hash, LogKind, Signature, TokenState};
use hypersync_solana_schema as schema;

/// Base58 of a 32-byte address filled with `n`.
fn addr58(n: u8) -> String {
    Address([n; 32]).to_string()
}

fn addr(n: u8) -> Address {
    Address([n; 32])
}

/// Base58 of a 32-byte hash filled with `n`.
fn hash58(n: u8) -> String {
    Hash([n; 32]).to_string()
}

fn hash(n: u8) -> Hash {
    Hash([n; 32])
}

/// Base58 of a 64-byte signature filled with `n`.
fn sig58(n: u8) -> String {
    Signature([n; 64]).to_string()
}

fn sig(n: u8) -> Signature {
    Signature([n; 64])
}

fn make_string_list(values: Vec<Vec<String>>) -> ListArray {
    let mut all_strings = Vec::new();
    let mut offsets = vec![0i32];
    for inner in &values {
        for s in inner {
            all_strings.push(Some(s.clone()));
        }
        offsets.push(all_strings.len() as i32);
    }
    let field = Arc::new(Field::new("item", DataType::Utf8, true));
    let inner = Arc::new(StringArray::from(all_strings));
    ListArray::new(field, OffsetBuffer::new(offsets.into()), inner, None)
}

fn make_u32_list(values: Vec<Vec<u32>>) -> ListArray {
    let mut all = Vec::new();
    let mut offsets = vec![0i32];
    for inner in &values {
        for n in inner {
            all.push(Some(*n));
        }
        offsets.push(all.len() as i32);
    }
    let field = Arc::new(Field::new("item", DataType::UInt32, true));
    let inner = Arc::new(UInt32Array::from(all));
    ListArray::new(field, OffsetBuffer::new(offsets.into()), inner, None)
}

#[test]
fn blocks_round_trip() {
    let batch = RecordBatch::try_new(
        schema::block(),
        vec![
            Arc::new(UInt64Array::from(vec![100u64, 101])),
            Arc::new(StringArray::from(vec![hash58(1), hash58(2)])),
            Arc::new(UInt64Array::from(vec![Some(99u64), None])),
            Arc::new(StringArray::from(vec![Some(hash58(3)), None])),
            Arc::new(Int64Array::from(vec![Some(1_700_000_000i64), None])),
            Arc::new(UInt64Array::from(vec![Some(50u64), Some(51)])),
        ],
    )
    .unwrap();
    let blocks = blocks_from_arrow(&batch).unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].slot, Some(100));
    assert_eq!(blocks[0].blockhash, Some(hash(1)));
    assert_eq!(blocks[0].parent_slot, Some(99));
    assert_eq!(blocks[0].parent_blockhash, Some(hash(3)));
    assert_eq!(blocks[0].block_time, Some(1_700_000_000));
    assert_eq!(blocks[1].parent_slot, None);
    assert_eq!(blocks[1].parent_blockhash, None);
    assert_eq!(blocks[1].block_height, Some(51));
}

#[test]
fn transactions_round_trip() {
    let batch = RecordBatch::try_new(
        schema::transaction(),
        vec![
            Arc::new(UInt64Array::from(vec![200u64])),
            Arc::new(UInt32Array::from(vec![3u32])),
            Arc::new(make_string_list(vec![vec![sig58(1), sig58(2)]])),
            Arc::new(StringArray::from(vec![Some(addr58(10))])),
            Arc::new(BooleanArray::from(vec![Some(true)])),
            Arc::new(StringArray::from(vec![None as Option<&str>])),
            Arc::new(UInt64Array::from(vec![Some(5_000u64)])),
            Arc::new(UInt64Array::from(vec![Some(12_345u64)])),
            Arc::new(make_string_list(vec![vec![
                addr58(11),
                addr58(12),
                addr58(13),
            ]])),
            Arc::new(StringArray::from(vec![Some(hash58(4))])),
            Arc::new(StringArray::from(vec![Some("legacy")])),
            Arc::new(make_string_list(vec![vec![addr58(14)]])),
            Arc::new(make_string_list(vec![vec![]])),
            Arc::new(BooleanArray::from(vec![Some(true)])),
        ],
    )
    .unwrap();
    let txs = transactions_from_arrow(&batch).unwrap();
    assert_eq!(txs.len(), 1);
    let tx = &txs[0];
    assert_eq!(tx.slot, Some(200));
    assert_eq!(tx.transaction_index, Some(3));
    assert_eq!(tx.signatures, Some(vec![sig(1), sig(2)]));
    assert_eq!(tx.fee_payer, Some(addr(10)));
    assert_eq!(tx.success, Some(true));
    assert_eq!(tx.err, None);
    assert_eq!(tx.fee, Some(5_000));
    assert_eq!(tx.compute_units_consumed, Some(12_345));
    assert_eq!(tx.account_keys, Some(vec![addr(11), addr(12), addr(13)]));
    assert_eq!(tx.recent_blockhash, Some(hash(4)));
    assert_eq!(tx.version.as_deref(), Some("legacy"));
    assert_eq!(tx.loaded_addresses_writable, Some(vec![addr(14)]));
    assert_eq!(tx.loaded_addresses_readonly, Some(vec![]));
    assert_eq!(tx.has_dropped_log_messages, Some(true));
}

#[test]
fn instruction_calls_round_trip() {
    let d1_arr = FixedSizeBinaryArray::try_from_sparse_iter_with_size(
        vec![Some(vec![0x0fu8]), None].into_iter(),
        1,
    )
    .unwrap();
    let d2_arr = FixedSizeBinaryArray::try_from_sparse_iter_with_size(
        vec![Some(vec![0x0fu8, 0x00]), None].into_iter(),
        2,
    )
    .unwrap();
    let d4_arr = FixedSizeBinaryArray::try_from_sparse_iter_with_size(
        vec![None::<Vec<u8>>, None].into_iter(),
        4,
    )
    .unwrap();
    let d8_arr = FixedSizeBinaryArray::try_from_sparse_iter_with_size(
        vec![Some(vec![1u8, 2, 3, 4, 5, 6, 7, 8]), None].into_iter(),
        8,
    )
    .unwrap();

    let batch = RecordBatch::try_new(
        schema::instruction_call(),
        vec![
            Arc::new(UInt64Array::from(vec![300u64, 301])),
            Arc::new(UInt32Array::from(vec![0u32, 1])),
            Arc::new(make_u32_list(vec![vec![0u32], vec![1, 0]])),
            Arc::new(StringArray::from(vec![addr58(20), addr58(21)])),
            Arc::new(make_string_list(vec![vec![addr58(30), addr58(31)], vec![]])),
            Arc::new(BinaryArray::from_opt_vec(vec![
                Some(&[0x0fu8, 0xaa, 0xbb] as &[u8]),
                None,
            ])),
            Arc::new(d1_arr),
            Arc::new(d2_arr),
            Arc::new(d4_arr),
            Arc::new(d8_arr),
            Arc::new(StringArray::from(vec![Some(addr58(30)), None])),
            Arc::new(StringArray::from(vec![Some(addr58(31)), None])),
            Arc::new(StringArray::from(vec![None, None] as Vec<Option<&str>>)),
            Arc::new(StringArray::from(vec![None, None] as Vec<Option<&str>>)),
            Arc::new(StringArray::from(vec![None, None] as Vec<Option<&str>>)),
            Arc::new(StringArray::from(vec![None, None] as Vec<Option<&str>>)),
            Arc::new(StringArray::from(vec![None, None] as Vec<Option<&str>>)),
            Arc::new(StringArray::from(vec![None, None] as Vec<Option<&str>>)),
            Arc::new(StringArray::from(vec![None, None] as Vec<Option<&str>>)),
            Arc::new(StringArray::from(vec![None, None] as Vec<Option<&str>>)),
            Arc::new(BooleanArray::from(vec![false, true])),
            Arc::new(BooleanArray::from(vec![true, true])),
            Arc::new(StringArray::from(vec![
                None,
                Some("custom program error: 0x1"),
            ])),
            Arc::new(UInt64Array::from(vec![Some(150u64), None])),
        ],
    )
    .unwrap();

    let ins = instruction_calls_from_arrow(&batch).unwrap();
    assert_eq!(ins.len(), 2);

    let outer = &ins[0];
    assert_eq!(outer.slot, Some(300));
    assert_eq!(outer.transaction_index, Some(0));
    assert_eq!(outer.instruction_address.as_deref(), Some(&[0u32][..]));
    assert_eq!(outer.stack_height(), Some(1));
    assert_eq!(outer.executing_account, Some(addr(20)));
    assert_eq!(outer.account_arguments, Some(vec![addr(30), addr(31)]));
    assert_eq!(outer.data.as_deref(), Some(&[0x0f, 0xaa, 0xbb][..]));
    assert_eq!(outer.d1.as_deref(), Some(&[0x0f][..]));
    assert_eq!(outer.d8.as_deref(), Some(&[1u8, 2, 3, 4, 5, 6, 7, 8][..]));
    assert_eq!(outer.a0, Some(addr(30)));
    assert_eq!(outer.a1, Some(addr(31)));
    assert_eq!(outer.is_inner, Some(false));
    assert_eq!(outer.tx_success, Some(true));
    assert_eq!(outer.error, None);
    assert_eq!(outer.compute_units_consumed, Some(150));

    let inner = &ins[1];
    assert_eq!(inner.instruction_address.as_deref(), Some(&[1u32, 0][..]));
    assert_eq!(inner.stack_height(), Some(2));
    assert_eq!(inner.account_arguments, Some(vec![]));
    assert_eq!(inner.data, None);
    assert_eq!(inner.d1, None);
    assert_eq!(inner.is_inner, Some(true));
    assert_eq!(inner.error.as_deref(), Some("custom program error: 0x1"));
    assert_eq!(inner.compute_units_consumed, None);
}

#[test]
fn logs_round_trip() {
    let batch = RecordBatch::try_new(
        schema::log(),
        vec![
            Arc::new(UInt64Array::from(vec![400u64, 400])),
            Arc::new(UInt32Array::from(vec![Some(2u32), Some(2)])),
            Arc::new(make_u32_list(vec![vec![0u32], vec![0]])),
            Arc::new(StringArray::from(vec![Some(addr58(5)), Some(addr58(5))])),
            Arc::new(StringArray::from(vec![Some("data"), Some("brand_new")])),
            Arc::new(StringArray::from(vec![
                Some("cGF5bG9hZA=="),
                Some("mystery line"),
            ])),
        ],
    )
    .unwrap();
    let logs = logs_from_arrow(&batch).unwrap();
    assert_eq!(logs.len(), 2);
    let l = &logs[0];
    assert_eq!(l.slot, Some(400));
    assert_eq!(l.transaction_index, Some(2));
    assert_eq!(l.instruction_address.as_deref(), Some(&[0u32][..]));
    assert_eq!(l.program_id, Some(addr(5)));
    assert_eq!(l.kind, Some(LogKind::Data));
    assert_eq!(l.message.as_deref(), Some("cGF5bG9hZA=="));
    // Unknown stored kinds fold into Other rather than failing the decode.
    assert_eq!(logs[1].kind, Some(LogKind::Other));
}

#[test]
fn account_activity_round_trip() {
    // A merged row (native + token) and a token-only row, so both sides of the
    // de-merge the table represents are decoded.
    let batch = RecordBatch::try_new(
        schema::account_activity(),
        vec![
            Arc::new(UInt64Array::from(vec![700u64, 700])),
            Arc::new(UInt32Array::from(vec![Some(1u32), Some(1u32)])),
            Arc::new(StringArray::from(vec![Some(sig58(9)), Some(sig58(9))])),
            Arc::new(UInt32Array::from(vec![Some(0u32), Some(3u32)])),
            Arc::new(StringArray::from(vec![Some(addr58(40)), Some(addr58(41))])),
            Arc::new(UInt64Array::from(vec![Some(100u64), None])),
            Arc::new(UInt64Array::from(vec![Some(99u64), None])),
            Arc::new(BooleanArray::from(vec![Some(true), Some(false)])),
            Arc::new(BooleanArray::from(vec![Some(true), Some(true)])),
            Arc::new(BooleanArray::from(vec![Some(true), Some(false)])),
            Arc::new(BooleanArray::from(vec![Some(false), Some(true)])),
            Arc::new(StringArray::from(vec![None, Some(addr58(42))])),
            Arc::new(StringArray::from(vec![None, Some(addr58(43))])),
            Arc::new(StringArray::from(vec![None, Some(addr58(44))])),
            Arc::new(UInt8Array::from(vec![None, Some(6u8)])),
            Arc::new(StringArray::from(vec![None, Some("1000")])),
            Arc::new(StringArray::from(vec![None, Some("900")])),
            Arc::new(StringArray::from(vec![
                None,
                Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            ])),
            Arc::new(StringArray::from(vec![
                None,
                Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            ])),
        ],
    )
    .unwrap();
    let rows = account_activity_from_arrow(&batch).unwrap();
    assert_eq!(rows.len(), 2);

    // Native side populated, token side absent.
    let native = &rows[0];
    assert_eq!(native.slot, Some(700));
    assert_eq!(native.transaction_id, Some(sig(9)));
    assert_eq!(native.account_index, Some(0));
    assert_eq!(native.pre_balance, Some(100));
    assert_eq!(native.post_balance, Some(99));
    assert_eq!(native.is_signer, Some(true));
    assert_eq!(native.is_fee_payer, Some(true));
    assert_eq!(native.from_lookup_table, Some(false));
    assert_eq!(native.mint, None);
    assert_eq!(native.token_decimals, None);

    // Token side populated, native side absent. Owner is split pre/post.
    let token = &rows[1];
    assert_eq!(token.account, Some(addr(41)));
    assert_eq!(token.pre_balance, None);
    assert_eq!(token.mint, Some(addr(42)));
    assert_eq!(token.pre_owner, Some(addr(43)));
    assert_eq!(token.post_owner, Some(addr(44)));
    assert_eq!(token.token_decimals, Some(6));
    assert_eq!(token.pre_token_balance, Some(1000));
    assert_eq!(token.post_token_balance, Some(900));
    assert_eq!(token.from_lookup_table, Some(true));
}

/// A projected response carries only the selected columns; the decoder must
/// fill the rest with None rather than failing.
#[test]
fn account_activity_decodes_projected_subset() {
    use arrow::datatypes::Schema;
    let projected = Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("account", DataType::Utf8, true),
        Field::new("mint", DataType::Utf8, true),
    ]));
    let batch = RecordBatch::try_new(
        projected,
        vec![
            Arc::new(UInt64Array::from(vec![800u64])),
            Arc::new(StringArray::from(vec![Some(addr58(50))])),
            Arc::new(StringArray::from(vec![Some(addr58(51))])),
        ],
    )
    .unwrap();
    let rows = account_activity_from_arrow(&batch).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].slot, Some(800));
    assert_eq!(rows[0].mint, Some(addr(51)));
    assert_eq!(rows[0].transaction_id, None);
    assert_eq!(rows[0].pre_balance, None);
}

/// The Q6 motivation: a grain-only projection of a table whose fields used to
/// be required must decode (this used to fail on `signatures`).
#[test]
fn transactions_decode_grain_only_projection() {
    use arrow::datatypes::Schema;
    let projected = Arc::new(Schema::new(vec![Field::new(
        "slot",
        DataType::UInt64,
        false,
    )]));
    let batch =
        RecordBatch::try_new(projected, vec![Arc::new(UInt64Array::from(vec![900u64]))]).unwrap();
    let txs = transactions_from_arrow(&batch).unwrap();
    assert_eq!(txs.len(), 1);
    assert_eq!(txs[0].slot, Some(900));
    assert_eq!(txs[0].signatures, None);
    assert_eq!(txs[0].success, None);
}

/// token_state is served as a Utf8 column; known values decode, unknown
/// values error (server-controlled closed set).
#[test]
fn account_activity_token_state_decodes() {
    use arrow::datatypes::Schema;
    let projected = Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("token_state", DataType::Utf8, true),
    ]));
    let batch = RecordBatch::try_new(
        projected.clone(),
        vec![
            Arc::new(UInt64Array::from(vec![1u64, 2, 3, 4])),
            Arc::new(StringArray::from(vec![
                Some("not_a_token"),
                Some("opened"),
                Some("closed"),
                Some("persisted"),
            ])),
        ],
    )
    .unwrap();
    let rows = account_activity_from_arrow(&batch).unwrap();
    assert_eq!(
        rows.iter().map(|r| r.token_state).collect::<Vec<_>>(),
        vec![
            Some(TokenState::NotAToken),
            Some(TokenState::Opened),
            Some(TokenState::Closed),
            Some(TokenState::Persisted),
        ]
    );

    let bad = RecordBatch::try_new(
        projected,
        vec![
            Arc::new(UInt64Array::from(vec![1u64])),
            Arc::new(StringArray::from(vec![Some("garbled")])),
        ],
    )
    .unwrap();
    assert!(account_activity_from_arrow(&bad).is_err());
}

/// A malformed base58 value in a pubkey column is corruption and must error,
/// not silently pass through.
#[test]
fn malformed_address_errors() {
    use arrow::datatypes::Schema;
    let projected = Arc::new(Schema::new(vec![
        Field::new("slot", DataType::UInt64, false),
        Field::new("account", DataType::Utf8, true),
    ]));
    let batch = RecordBatch::try_new(
        projected,
        vec![
            Arc::new(UInt64Array::from(vec![1u64])),
            Arc::new(StringArray::from(vec![Some("not-base58!")])),
        ],
    )
    .unwrap();
    let err = account_activity_from_arrow(&batch).unwrap_err();
    assert!(err.to_string().contains("account"), "{err}");
}

#[test]
fn rewards_round_trip() {
    let batch = RecordBatch::try_new(
        schema::reward(),
        vec![
            Arc::new(UInt64Array::from(vec![700u64])),
            Arc::new(StringArray::from(vec![Some(addr58(60))])),
            Arc::new(Int64Array::from(vec![Some(42i64)])),
            Arc::new(UInt64Array::from(vec![Some(1234u64)])),
            Arc::new(StringArray::from(vec![Some("staking")])),
            Arc::new(UInt8Array::from(vec![Some(10u8)])),
        ],
    )
    .unwrap();
    let r = rewards_from_arrow(&batch).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].pubkey, Some(addr(60)));
    assert_eq!(r[0].lamports, Some(42));
    assert_eq!(r[0].commission, Some(10));
}

/// All-Option decode: a batch carrying an unrelated subset of columns decodes
/// with every unselected field None (this used to be a hard error for
/// blocks, which required slot + blockhash).
#[test]
fn missing_columns_decode_as_none() {
    let schema = Arc::new(arrow::datatypes::Schema::new(vec![Field::new(
        "blockhash",
        DataType::Utf8,
        false,
    )]));
    let batch =
        RecordBatch::try_new(schema, vec![Arc::new(StringArray::from(vec![hash58(7)]))]).unwrap();
    let blocks = blocks_from_arrow(&batch).unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].slot, None);
    assert_eq!(blocks[0].blockhash, Some(hash(7)));
}

#[test]
fn empty_batch_returns_empty_vec() {
    let batch = RecordBatch::new_empty(schema::block());
    assert!(blocks_from_arrow(&batch).unwrap().is_empty());
}
