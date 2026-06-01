//! Decode Arrow `RecordBatch`es into the typed structs in [`simple_types`].
//!
//! Column lookup is by name, not by index, so a schema drift on the server
//! surfaces as a clear "column not found" error rather than a silent miscast.

use anyhow::{anyhow, Context, Result};
use arrow::array::{
    Array, BinaryArray, BooleanArray, FixedSizeBinaryArray, Int64Array, ListArray, StringArray,
    UInt32Array, UInt64Array, UInt8Array,
};
use arrow::record_batch::RecordBatch;

use crate::simple_types::{Balance, Block, Instruction, Log, Reward, TokenBalance, Transaction};

// ---------- column accessor helpers ----------

fn col<'a, T: 'static>(batch: &'a RecordBatch, name: &str) -> Result<&'a T> {
    let arr = batch
        .column_by_name(name)
        .ok_or_else(|| anyhow!("column {} missing from batch", name))?;
    arr.as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| anyhow!("column {} has unexpected arrow type", name))
}

fn opt_col<'a, T: 'static>(batch: &'a RecordBatch, name: &str) -> Result<Option<&'a T>> {
    match batch.column_by_name(name) {
        None => Ok(None),
        Some(arr) => {
            Ok(Some(arr.as_any().downcast_ref::<T>().ok_or_else(|| {
                anyhow!("column {} has unexpected arrow type", name)
            })?))
        }
    }
}

fn get_u64(arr: &UInt64Array, i: usize) -> Option<u64> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i))
    }
}

fn get_u32(arr: &UInt32Array, i: usize) -> Option<u32> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i))
    }
}

fn get_i64(arr: &Int64Array, i: usize) -> Option<i64> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i))
    }
}

fn get_u8(arr: &UInt8Array, i: usize) -> Option<u8> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i))
    }
}

fn get_bool(arr: &BooleanArray, i: usize) -> Option<bool> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i))
    }
}

fn get_str(arr: &StringArray, i: usize) -> Option<String> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i).to_owned())
    }
}

fn get_fixed_bytes(arr: &FixedSizeBinaryArray, i: usize) -> Option<Vec<u8>> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i).to_vec())
    }
}

fn get_bytes(arr: &BinaryArray, i: usize) -> Vec<u8> {
    if arr.is_null(i) {
        Vec::new()
    } else {
        arr.value(i).to_vec()
    }
}

/// Decode `ListArray<Utf8>` at row `i` into a `Vec<String>`. Null entries in the
/// inner list become empty strings so the positional layout is preserved.
fn get_string_list(arr: &ListArray, i: usize) -> Result<Vec<String>> {
    if arr.is_null(i) {
        return Ok(Vec::new());
    }
    let values = arr.value(i);
    let strings = values
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| anyhow!("expected Utf8 list values"))?;
    let mut out = Vec::with_capacity(strings.len());
    for j in 0..strings.len() {
        if strings.is_null(j) {
            out.push(String::new());
        } else {
            out.push(strings.value(j).to_owned());
        }
    }
    Ok(out)
}

/// Decode `ListArray<UInt32>` at row `i` into a `Vec<u32>`. Null entries in the
/// inner list become 0 to preserve positional layout.
fn get_u32_list(arr: &ListArray, i: usize) -> Result<Vec<u32>> {
    if arr.is_null(i) {
        return Ok(Vec::new());
    }
    let values = arr.value(i);
    let u32s = values
        .as_any()
        .downcast_ref::<UInt32Array>()
        .ok_or_else(|| anyhow!("expected UInt32 list values"))?;
    let mut out = Vec::with_capacity(u32s.len());
    for j in 0..u32s.len() {
        if u32s.is_null(j) {
            out.push(0);
        } else {
            out.push(u32s.value(j));
        }
    }
    Ok(out)
}

// ---------- per-table decoders ----------

pub fn blocks_from_arrow(batch: &RecordBatch) -> Result<Vec<Block>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = col::<UInt64Array>(batch, "slot")?;
    let blockhash = col::<StringArray>(batch, "blockhash")?;
    let parent_slot = opt_col::<UInt64Array>(batch, "parent_slot")?;
    let parent_blockhash = opt_col::<StringArray>(batch, "parent_blockhash")?;
    let block_time = opt_col::<Int64Array>(batch, "block_time")?;
    let block_height = opt_col::<UInt64Array>(batch, "block_height")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(Block {
            slot: slot.value(i),
            blockhash: blockhash.value(i).to_owned(),
            parent_slot: parent_slot.and_then(|a| get_u64(a, i)),
            parent_blockhash: parent_blockhash.and_then(|a| get_str(a, i)),
            block_time: block_time.and_then(|a| get_i64(a, i)),
            block_height: block_height.and_then(|a| get_u64(a, i)),
        });
    }
    Ok(out)
}

pub fn transactions_from_arrow(batch: &RecordBatch) -> Result<Vec<Transaction>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = col::<UInt64Array>(batch, "slot")?;
    let tx_index = col::<UInt32Array>(batch, "transaction_index")?;
    let signatures = col::<ListArray>(batch, "signatures")?;
    let fee_payer = opt_col::<StringArray>(batch, "fee_payer")?;
    let success = opt_col::<BooleanArray>(batch, "success")?;
    let err = opt_col::<StringArray>(batch, "err")?;
    let fee = opt_col::<UInt64Array>(batch, "fee")?;
    let compute = opt_col::<UInt64Array>(batch, "compute_units_consumed")?;
    let account_keys = opt_col::<ListArray>(batch, "account_keys")?;
    let recent_blockhash = opt_col::<StringArray>(batch, "recent_blockhash")?;
    let version = opt_col::<StringArray>(batch, "version")?;
    let loaded_w = opt_col::<ListArray>(batch, "loaded_addresses_writable")?;
    let loaded_r = opt_col::<ListArray>(batch, "loaded_addresses_readonly")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(Transaction {
            slot: slot.value(i),
            transaction_index: tx_index.value(i),
            signatures: get_string_list(signatures, i).context("transactions.signatures")?,
            fee_payer: fee_payer.and_then(|a| get_str(a, i)),
            success: success.and_then(|a| get_bool(a, i)),
            err: err.and_then(|a| get_str(a, i)),
            fee: fee.and_then(|a| get_u64(a, i)),
            compute_units_consumed: compute.and_then(|a| get_u64(a, i)),
            account_keys: match account_keys {
                Some(a) => get_string_list(a, i).context("transactions.account_keys")?,
                None => Vec::new(),
            },
            recent_blockhash: recent_blockhash.and_then(|a| get_str(a, i)),
            version: version.and_then(|a| get_str(a, i)),
            loaded_addresses_writable: match loaded_w {
                Some(a) => {
                    get_string_list(a, i).context("transactions.loaded_addresses_writable")?
                }
                None => Vec::new(),
            },
            loaded_addresses_readonly: match loaded_r {
                Some(a) => {
                    get_string_list(a, i).context("transactions.loaded_addresses_readonly")?
                }
                None => Vec::new(),
            },
        });
    }
    Ok(out)
}

pub fn instructions_from_arrow(batch: &RecordBatch) -> Result<Vec<Instruction>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = col::<UInt64Array>(batch, "slot")?;
    let tx_index = col::<UInt32Array>(batch, "transaction_index")?;
    let inst_addr = col::<ListArray>(batch, "instruction_address")?;
    let program_id = col::<StringArray>(batch, "program_id")?;
    let accounts = opt_col::<ListArray>(batch, "accounts")?;
    let data = opt_col::<BinaryArray>(batch, "data")?;
    let d1 = opt_col::<FixedSizeBinaryArray>(batch, "d1")?;
    let d2 = opt_col::<FixedSizeBinaryArray>(batch, "d2")?;
    let d4 = opt_col::<FixedSizeBinaryArray>(batch, "d4")?;
    let d8 = opt_col::<FixedSizeBinaryArray>(batch, "d8")?;
    let a = [
        opt_col::<StringArray>(batch, "a0")?,
        opt_col::<StringArray>(batch, "a1")?,
        opt_col::<StringArray>(batch, "a2")?,
        opt_col::<StringArray>(batch, "a3")?,
        opt_col::<StringArray>(batch, "a4")?,
        opt_col::<StringArray>(batch, "a5")?,
        opt_col::<StringArray>(batch, "a6")?,
        opt_col::<StringArray>(batch, "a7")?,
        opt_col::<StringArray>(batch, "a8")?,
        opt_col::<StringArray>(batch, "a9")?,
    ];
    let is_inner = col::<BooleanArray>(batch, "is_inner")?;
    let is_committed = col::<BooleanArray>(batch, "is_committed")?;

    let ax = |i: usize, pos: usize| a[pos].and_then(|arr| get_str(arr, i));

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(Instruction {
            slot: slot.value(i),
            transaction_index: tx_index.value(i),
            instruction_address: get_u32_list(inst_addr, i)
                .context("instructions.instruction_address")?,
            program_id: program_id.value(i).to_owned(),
            accounts: match accounts {
                Some(a) => get_string_list(a, i).context("instructions.accounts")?,
                None => Vec::new(),
            },
            data: data.map(|a| get_bytes(a, i)).unwrap_or_default(),
            d1: d1.and_then(|arr| get_fixed_bytes(arr, i)),
            d2: d2.and_then(|arr| get_fixed_bytes(arr, i)),
            d4: d4.and_then(|arr| get_fixed_bytes(arr, i)),
            d8: d8.and_then(|arr| get_fixed_bytes(arr, i)),
            a0: ax(i, 0),
            a1: ax(i, 1),
            a2: ax(i, 2),
            a3: ax(i, 3),
            a4: ax(i, 4),
            a5: ax(i, 5),
            a6: ax(i, 6),
            a7: ax(i, 7),
            a8: ax(i, 8),
            a9: ax(i, 9),
            is_inner: is_inner.value(i),
            is_committed: is_committed.value(i),
        });
    }
    Ok(out)
}

pub fn logs_from_arrow(batch: &RecordBatch) -> Result<Vec<Log>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = col::<UInt64Array>(batch, "slot")?;
    let tx_index = opt_col::<UInt32Array>(batch, "transaction_index")?;
    let inst_addr = opt_col::<ListArray>(batch, "instruction_address")?;
    let program_id = opt_col::<StringArray>(batch, "program_id")?;
    let kind = opt_col::<StringArray>(batch, "kind")?;
    let message = opt_col::<StringArray>(batch, "message")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let inst_addr_v = match inst_addr {
            Some(a) if !a.is_null(i) => {
                Some(get_u32_list(a, i).context("logs.instruction_address")?)
            }
            _ => None,
        };
        out.push(Log {
            slot: slot.value(i),
            transaction_index: tx_index.and_then(|a| get_u32(a, i)),
            instruction_address: inst_addr_v,
            program_id: program_id.and_then(|a| get_str(a, i)),
            kind: kind.and_then(|a| get_str(a, i)),
            message: message.and_then(|a| get_str(a, i)),
        });
    }
    Ok(out)
}

pub fn balances_from_arrow(batch: &RecordBatch) -> Result<Vec<Balance>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = col::<UInt64Array>(batch, "slot")?;
    let tx_index = opt_col::<UInt32Array>(batch, "transaction_index")?;
    let account = opt_col::<StringArray>(batch, "account")?;
    let pre = opt_col::<UInt64Array>(batch, "pre")?;
    let post = opt_col::<UInt64Array>(batch, "post")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(Balance {
            slot: slot.value(i),
            transaction_index: tx_index.and_then(|a| get_u32(a, i)),
            account: account.and_then(|a| get_str(a, i)),
            pre: pre.and_then(|a| get_u64(a, i)),
            post: post.and_then(|a| get_u64(a, i)),
        });
    }
    Ok(out)
}

pub fn token_balances_from_arrow(batch: &RecordBatch) -> Result<Vec<TokenBalance>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = col::<UInt64Array>(batch, "slot")?;
    let tx_index = opt_col::<UInt32Array>(batch, "transaction_index")?;
    let account = opt_col::<StringArray>(batch, "account")?;
    let mint = opt_col::<StringArray>(batch, "mint")?;
    let owner = opt_col::<StringArray>(batch, "owner")?;
    let pre = opt_col::<StringArray>(batch, "pre_amount")?;
    let post = opt_col::<StringArray>(batch, "post_amount")?;
    // Optional so responses from servers predating program-id capture still decode.
    let pre_program_id = opt_col::<StringArray>(batch, "pre_program_id")?;
    let post_program_id = opt_col::<StringArray>(batch, "post_program_id")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(TokenBalance {
            slot: slot.value(i),
            transaction_index: tx_index.and_then(|a| get_u32(a, i)),
            account: account.and_then(|a| get_str(a, i)),
            mint: mint.and_then(|a| get_str(a, i)),
            owner: owner.and_then(|a| get_str(a, i)),
            pre_amount: pre.and_then(|a| get_str(a, i)),
            post_amount: post.and_then(|a| get_str(a, i)),
            pre_program_id: pre_program_id.and_then(|a| get_str(a, i)),
            post_program_id: post_program_id.and_then(|a| get_str(a, i)),
        });
    }
    Ok(out)
}

pub fn rewards_from_arrow(batch: &RecordBatch) -> Result<Vec<Reward>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = col::<UInt64Array>(batch, "slot")?;
    let pubkey = opt_col::<StringArray>(batch, "pubkey")?;
    let lamports = opt_col::<Int64Array>(batch, "lamports")?;
    let post_balance = opt_col::<UInt64Array>(batch, "post_balance")?;
    let reward_type = opt_col::<StringArray>(batch, "reward_type")?;
    let commission = opt_col::<UInt8Array>(batch, "commission")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(Reward {
            slot: slot.value(i),
            pubkey: pubkey.and_then(|a| get_str(a, i)),
            lamports: lamports.and_then(|a| get_i64(a, i)),
            post_balance: post_balance.and_then(|a| get_u64(a, i)),
            reward_type: reward_type.and_then(|a| get_str(a, i)),
            commission: commission.and_then(|a| get_u8(a, i)),
        });
    }
    Ok(out)
}
