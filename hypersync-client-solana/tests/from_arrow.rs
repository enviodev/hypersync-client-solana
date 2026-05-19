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
use hypersync_solana_schema as schema;

fn make_string_list(values: Vec<Vec<&str>>) -> ListArray {
    let mut all_strings = Vec::new();
    let mut offsets = vec![0i32];
    for inner in &values {
        for s in inner {
            all_strings.push(Some(s.to_string()));
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
            Arc::new(StringArray::from(vec!["bh-100", "bh-101"])),
            Arc::new(UInt64Array::from(vec![Some(99u64), None])),
            Arc::new(StringArray::from(vec![Some("parent-99"), None])),
            Arc::new(Int64Array::from(vec![Some(1_700_000_000i64), None])),
            Arc::new(UInt64Array::from(vec![Some(50u64), Some(51)])),
        ],
    )
    .unwrap();
    let blocks = blocks_from_arrow(&batch).unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].slot, 100);
    assert_eq!(blocks[0].blockhash, "bh-100");
    assert_eq!(blocks[0].parent_slot, Some(99));
    assert_eq!(blocks[0].parent_blockhash.as_deref(), Some("parent-99"));
    assert_eq!(blocks[0].block_time, Some(1_700_000_000));
    assert_eq!(blocks[1].parent_slot, None);
    assert_eq!(blocks[1].block_height, Some(51));
}

#[test]
fn transactions_round_trip() {
    let batch = RecordBatch::try_new(
        schema::transaction(),
        vec![
            Arc::new(UInt64Array::from(vec![200u64])),
            Arc::new(UInt32Array::from(vec![3u32])),
            Arc::new(make_string_list(vec![vec!["sig-a", "sig-b"]])),
            Arc::new(StringArray::from(vec![Some("fee-payer-x")])),
            Arc::new(BooleanArray::from(vec![Some(true)])),
            Arc::new(StringArray::from(vec![None as Option<&str>])),
            Arc::new(UInt64Array::from(vec![Some(5_000u64)])),
            Arc::new(UInt64Array::from(vec![Some(12_345u64)])),
            Arc::new(make_string_list(vec![vec!["acc-1", "acc-2", "acc-3"]])),
            Arc::new(StringArray::from(vec![Some("recent-hash")])),
            Arc::new(StringArray::from(vec![Some("legacy")])),
            Arc::new(make_string_list(vec![vec!["loaded-w-1"]])),
            Arc::new(make_string_list(vec![vec![]])),
        ],
    )
    .unwrap();
    let txs = transactions_from_arrow(&batch).unwrap();
    assert_eq!(txs.len(), 1);
    let tx = &txs[0];
    assert_eq!(tx.slot, 200);
    assert_eq!(tx.transaction_index, 3);
    assert_eq!(tx.signatures, vec!["sig-a", "sig-b"]);
    assert_eq!(tx.fee_payer.as_deref(), Some("fee-payer-x"));
    assert_eq!(tx.success, Some(true));
    assert_eq!(tx.err, None);
    assert_eq!(tx.fee, Some(5_000));
    assert_eq!(tx.compute_units_consumed, Some(12_345));
    assert_eq!(tx.account_keys.len(), 3);
    assert_eq!(tx.version.as_deref(), Some("legacy"));
    assert_eq!(tx.loaded_addresses_writable, vec!["loaded-w-1"]);
    assert!(tx.loaded_addresses_readonly.is_empty());
}

#[test]
fn instructions_round_trip() {
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
        schema::instruction(),
        vec![
            Arc::new(UInt64Array::from(vec![300u64, 301])),
            Arc::new(UInt32Array::from(vec![0u32, 1])),
            Arc::new(make_u32_list(vec![vec![0u32], vec![1, 0]])),
            Arc::new(StringArray::from(vec!["prog-a", "prog-b"])),
            Arc::new(make_string_list(vec![vec!["acc-0", "acc-1"], vec![]])),
            Arc::new(BinaryArray::from_opt_vec(vec![
                Some(&[0x0fu8, 0xaa, 0xbb] as &[u8]),
                None,
            ])),
            Arc::new(d1_arr),
            Arc::new(d2_arr),
            Arc::new(d4_arr),
            Arc::new(d8_arr),
            Arc::new(StringArray::from(vec![Some("acc-0"), None])),
            Arc::new(StringArray::from(vec![Some("acc-1"), None])),
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
        ],
    )
    .unwrap();

    let ins = instructions_from_arrow(&batch).unwrap();
    assert_eq!(ins.len(), 2);

    let outer = &ins[0];
    assert_eq!(outer.slot, 300);
    assert_eq!(outer.transaction_index, 0);
    assert_eq!(outer.instruction_address, vec![0]);
    assert_eq!(outer.program_id, "prog-a");
    assert_eq!(outer.accounts, vec!["acc-0", "acc-1"]);
    assert_eq!(outer.data, vec![0x0f, 0xaa, 0xbb]);
    assert_eq!(outer.d1.as_deref(), Some(&[0x0f][..]));
    assert_eq!(outer.d8.as_deref(), Some(&[1u8, 2, 3, 4, 5, 6, 7, 8][..]));
    assert_eq!(outer.a0.as_deref(), Some("acc-0"));
    assert_eq!(outer.a1.as_deref(), Some("acc-1"));
    assert!(!outer.is_inner);
    assert!(outer.is_committed);

    let inner = &ins[1];
    assert_eq!(inner.instruction_address, vec![1, 0]);
    assert!(inner.accounts.is_empty());
    assert_eq!(inner.data, Vec::<u8>::new());
    assert_eq!(inner.d1, None);
    assert!(inner.is_inner);
}

#[test]
fn logs_round_trip() {
    let batch = RecordBatch::try_new(
        schema::log(),
        vec![
            Arc::new(UInt64Array::from(vec![400u64])),
            Arc::new(UInt32Array::from(vec![Some(2u32)])),
            Arc::new(make_u32_list(vec![vec![0u32]])),
            Arc::new(StringArray::from(vec![Some("prog")])),
            Arc::new(StringArray::from(vec![Some("data")])),
            Arc::new(StringArray::from(vec![Some("Program log: hello")])),
        ],
    )
    .unwrap();
    let logs = logs_from_arrow(&batch).unwrap();
    assert_eq!(logs.len(), 1);
    let l = &logs[0];
    assert_eq!(l.slot, 400);
    assert_eq!(l.transaction_index, Some(2));
    assert_eq!(l.instruction_address.as_deref(), Some(&[0u32][..]));
    assert_eq!(l.program_id.as_deref(), Some("prog"));
    assert_eq!(l.kind.as_deref(), Some("data"));
    assert_eq!(l.message.as_deref(), Some("Program log: hello"));
}

#[test]
fn balances_round_trip() {
    let batch = RecordBatch::try_new(
        schema::balance(),
        vec![
            Arc::new(UInt64Array::from(vec![500u64])),
            Arc::new(UInt32Array::from(vec![Some(0u32)])),
            Arc::new(StringArray::from(vec![Some("acc")])),
            Arc::new(UInt64Array::from(vec![Some(100u64)])),
            Arc::new(UInt64Array::from(vec![Some(99u64)])),
        ],
    )
    .unwrap();
    let bal = balances_from_arrow(&batch).unwrap();
    assert_eq!(bal.len(), 1);
    assert_eq!(bal[0].pre, Some(100));
    assert_eq!(bal[0].post, Some(99));
}

#[test]
fn token_balances_round_trip() {
    let batch = RecordBatch::try_new(
        schema::token_balance(),
        vec![
            Arc::new(UInt64Array::from(vec![600u64])),
            Arc::new(UInt32Array::from(vec![Some(0u32)])),
            Arc::new(StringArray::from(vec![Some("ata")])),
            Arc::new(StringArray::from(vec![Some("mint")])),
            Arc::new(StringArray::from(vec![Some("owner")])),
            Arc::new(StringArray::from(vec![Some("1000")])),
            Arc::new(StringArray::from(vec![Some("900")])),
        ],
    )
    .unwrap();
    let tb = token_balances_from_arrow(&batch).unwrap();
    assert_eq!(tb.len(), 1);
    assert_eq!(tb[0].mint.as_deref(), Some("mint"));
    assert_eq!(tb[0].pre_amount.as_deref(), Some("1000"));
}

#[test]
fn rewards_round_trip() {
    let batch = RecordBatch::try_new(
        schema::reward(),
        vec![
            Arc::new(UInt64Array::from(vec![700u64])),
            Arc::new(StringArray::from(vec![Some("validator")])),
            Arc::new(Int64Array::from(vec![Some(42i64)])),
            Arc::new(UInt64Array::from(vec![Some(1234u64)])),
            Arc::new(StringArray::from(vec![Some("staking")])),
            Arc::new(UInt8Array::from(vec![Some(10u8)])),
        ],
    )
    .unwrap();
    let r = rewards_from_arrow(&batch).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].lamports, Some(42));
    assert_eq!(r[0].commission, Some(10));
}

#[test]
fn missing_required_column_errors() {
    let schema = Arc::new(arrow::datatypes::Schema::new(vec![Field::new(
        "blockhash",
        DataType::Utf8,
        false,
    )]));
    let batch = RecordBatch::try_new(schema, vec![Arc::new(StringArray::from(vec!["x"]))]).unwrap();
    let err = blocks_from_arrow(&batch).unwrap_err();
    assert!(err.to_string().contains("slot"));
}

#[test]
fn empty_batch_returns_empty_vec() {
    let batch = RecordBatch::new_empty(schema::block());
    assert!(blocks_from_arrow(&batch).unwrap().is_empty());
}
