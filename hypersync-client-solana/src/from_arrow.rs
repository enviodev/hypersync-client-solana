//! Decode Arrow `RecordBatch`es into the typed structs in [`simple_types`].
//!
//! Column lookup is by name. Every column is optional (`opt_col`): with
//! `field_selection` any column can be projected away, and a projected query
//! must still decode, leaving the absent fields `None`. A column that IS
//! present but holds malformed data (non-base58 pubkey, non-numeric token
//! amount) errors loudly: that is corruption, not projection.

use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use arrow::array::{
    Array, BinaryArray, BooleanArray, FixedSizeBinaryArray, Int64Array, ListArray, StringArray,
    UInt32Array, UInt64Array, UInt8Array,
};
use arrow::record_batch::RecordBatch;

use crate::simple_types::{
    AccountActivity, Address, Block, Hash, InstructionCall, Log, LogKind, Reward, Signature,
    TokenState, Transaction,
};

// ---------- column accessor helpers ----------

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
    (!arr.is_null(i)).then(|| arr.value(i))
}

fn get_u32(arr: &UInt32Array, i: usize) -> Option<u32> {
    (!arr.is_null(i)).then(|| arr.value(i))
}

fn get_i64(arr: &Int64Array, i: usize) -> Option<i64> {
    (!arr.is_null(i)).then(|| arr.value(i))
}

fn get_u8(arr: &UInt8Array, i: usize) -> Option<u8> {
    (!arr.is_null(i)).then(|| arr.value(i))
}

fn get_bool(arr: &BooleanArray, i: usize) -> Option<bool> {
    (!arr.is_null(i)).then(|| arr.value(i))
}

fn get_str(arr: &StringArray, i: usize) -> Option<String> {
    (!arr.is_null(i)).then(|| arr.value(i).to_owned())
}

/// Parse a stored base58 string into one of the byte newtypes. A malformed
/// value is corruption and errors loudly rather than passing through.
fn get_parsed<T>(arr: &StringArray, i: usize, column: &str) -> Result<Option<T>>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    if arr.is_null(i) {
        return Ok(None);
    }
    let s = arr.value(i);
    s.parse()
        .map(Some)
        .map_err(|e| anyhow!("column {column} row {i}: {e} (value {s:?})"))
}

/// Parse a stored decimal string into a u64 (raw SPL token amounts are u64
/// on-chain). A non-numeric value indicates source corruption: error loudly.
fn get_u64_from_str(arr: &StringArray, i: usize, column: &str) -> Result<Option<u64>> {
    if arr.is_null(i) {
        return Ok(None);
    }
    let s = arr.value(i);
    s.parse()
        .map(Some)
        .with_context(|| format!("column {column} row {i}: non-u64 amount {s:?}"))
}

fn get_fixed_bytes(arr: &FixedSizeBinaryArray, i: usize) -> Option<Vec<u8>> {
    (!arr.is_null(i)).then(|| arr.value(i).to_vec())
}

fn get_bytes(arr: &BinaryArray, i: usize) -> Option<Vec<u8>> {
    (!arr.is_null(i)).then(|| arr.value(i).to_vec())
}

/// Decode `ListArray<Utf8>` at row `i`, parsing each entry with `FromStr`.
/// A null row decodes as `None`; a null or malformed ENTRY is an error (the
/// positional layout of these lists is meaningful, so a hole is corruption).
fn get_parsed_list<T>(arr: &ListArray, i: usize, column: &str) -> Result<Option<Vec<T>>>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    if arr.is_null(i) {
        return Ok(None);
    }
    let values = arr.value(i);
    let strings = values
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| anyhow!("column {column}: expected Utf8 list values"))?;
    let mut out = Vec::with_capacity(strings.len());
    for j in 0..strings.len() {
        if strings.is_null(j) {
            return Err(anyhow!(
                "column {column} row {i}: null entry at position {j}"
            ));
        }
        let s = strings.value(j);
        out.push(
            s.parse()
                .map_err(|e| anyhow!("column {column} row {i} entry {j}: {e} (value {s:?})"))?,
        );
    }
    Ok(Some(out))
}

/// Decode `ListArray<UInt32>` at row `i` into a `Vec<u32>`. A null row decodes
/// as `None`; a null entry is an error (positions are meaningful).
fn get_u32_list(arr: &ListArray, i: usize, column: &str) -> Result<Option<Vec<u32>>> {
    if arr.is_null(i) {
        return Ok(None);
    }
    let values = arr.value(i);
    let u32s = values
        .as_any()
        .downcast_ref::<UInt32Array>()
        .ok_or_else(|| anyhow!("column {column}: expected UInt32 list values"))?;
    let mut out = Vec::with_capacity(u32s.len());
    for j in 0..u32s.len() {
        if u32s.is_null(j) {
            return Err(anyhow!(
                "column {column} row {i}: null entry at position {j}"
            ));
        }
        out.push(u32s.value(j));
    }
    Ok(Some(out))
}

// ---------- per-table decoders ----------

pub fn blocks_from_arrow(batch: &RecordBatch) -> Result<Vec<Block>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = opt_col::<UInt64Array>(batch, "slot")?;
    let blockhash = opt_col::<StringArray>(batch, "blockhash")?;
    let parent_slot = opt_col::<UInt64Array>(batch, "parent_slot")?;
    let parent_blockhash = opt_col::<StringArray>(batch, "parent_blockhash")?;
    let block_time = opt_col::<Int64Array>(batch, "block_time")?;
    let block_height = opt_col::<UInt64Array>(batch, "block_height")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(Block {
            slot: slot.and_then(|a| get_u64(a, i)),
            blockhash: blockhash
                .map(|a| get_parsed::<Hash>(a, i, "blocks.blockhash"))
                .transpose()?
                .flatten(),
            parent_slot: parent_slot.and_then(|a| get_u64(a, i)),
            parent_blockhash: parent_blockhash
                .map(|a| get_parsed::<Hash>(a, i, "blocks.parent_blockhash"))
                .transpose()?
                .flatten(),
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
    let slot = opt_col::<UInt64Array>(batch, "slot")?;
    let tx_index = opt_col::<UInt32Array>(batch, "transaction_index")?;
    let transaction_id = opt_col::<StringArray>(batch, "transaction_id")?;
    let signatures = opt_col::<ListArray>(batch, "signatures")?;
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
    let dropped_logs = opt_col::<BooleanArray>(batch, "has_dropped_log_messages")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(Transaction {
            slot: slot.and_then(|a| get_u64(a, i)),
            transaction_index: tx_index.and_then(|a| get_u32(a, i)),
            transaction_id: transaction_id
                .map(|a| get_parsed::<Signature>(a, i, "transactions.transaction_id"))
                .transpose()?
                .flatten(),
            signatures: signatures
                .map(|a| get_parsed_list::<Signature>(a, i, "transactions.signatures"))
                .transpose()?
                .flatten(),
            fee_payer: fee_payer
                .map(|a| get_parsed::<Address>(a, i, "transactions.fee_payer"))
                .transpose()?
                .flatten(),
            success: success.and_then(|a| get_bool(a, i)),
            err: err.and_then(|a| get_str(a, i)),
            fee: fee.and_then(|a| get_u64(a, i)),
            compute_units_consumed: compute.and_then(|a| get_u64(a, i)),
            account_keys: account_keys
                .map(|a| get_parsed_list::<Address>(a, i, "transactions.account_keys"))
                .transpose()?
                .flatten(),
            recent_blockhash: recent_blockhash
                .map(|a| get_parsed::<Hash>(a, i, "transactions.recent_blockhash"))
                .transpose()?
                .flatten(),
            version: version.and_then(|a| get_str(a, i)),
            loaded_addresses_writable: loaded_w
                .map(|a| get_parsed_list::<Address>(a, i, "transactions.loaded_addresses_writable"))
                .transpose()?
                .flatten(),
            loaded_addresses_readonly: loaded_r
                .map(|a| get_parsed_list::<Address>(a, i, "transactions.loaded_addresses_readonly"))
                .transpose()?
                .flatten(),
            has_dropped_log_messages: dropped_logs.and_then(|a| get_bool(a, i)),
        });
    }
    Ok(out)
}

pub fn instruction_calls_from_arrow(batch: &RecordBatch) -> Result<Vec<InstructionCall>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = opt_col::<UInt64Array>(batch, "slot")?;
    let tx_index = opt_col::<UInt32Array>(batch, "transaction_index")?;
    let inst_addr = opt_col::<ListArray>(batch, "instruction_address")?;
    let executing_account = opt_col::<StringArray>(batch, "executing_account")?;
    let executing_account_index = opt_col::<UInt32Array>(batch, "executing_account_index")?;
    let account_arguments = opt_col::<ListArray>(batch, "account_arguments")?;
    let account_index_arguments = opt_col::<ListArray>(batch, "account_index_arguments")?;
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
    let is_inner = opt_col::<BooleanArray>(batch, "is_inner")?;
    let tx_success = opt_col::<BooleanArray>(batch, "tx_success")?;
    let error = opt_col::<StringArray>(batch, "error")?;
    let compute = opt_col::<UInt64Array>(batch, "compute_units_consumed")?;

    let ax = |i: usize, pos: usize| -> Result<Option<Address>> {
        a[pos]
            .map(|arr| get_parsed::<Address>(arr, i, "instruction_calls.a*"))
            .transpose()
            .map(Option::flatten)
    };

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(InstructionCall {
            slot: slot.and_then(|a| get_u64(a, i)),
            transaction_index: tx_index.and_then(|a| get_u32(a, i)),
            instruction_address: inst_addr
                .map(|a| get_u32_list(a, i, "instruction_calls.instruction_address"))
                .transpose()?
                .flatten(),
            executing_account: executing_account
                .map(|a| get_parsed::<Address>(a, i, "instruction_calls.executing_account"))
                .transpose()?
                .flatten(),
            executing_account_index: executing_account_index.and_then(|a| get_u32(a, i)),
            account_arguments: account_arguments
                .map(|a| get_parsed_list::<Address>(a, i, "instruction_calls.account_arguments"))
                .transpose()?
                .flatten(),
            account_index_arguments: account_index_arguments
                .map(|a| get_u32_list(a, i, "instruction_calls.account_index_arguments"))
                .transpose()?
                .flatten(),
            data: data.and_then(|a| get_bytes(a, i)),
            d1: d1.and_then(|arr| get_fixed_bytes(arr, i)),
            d2: d2.and_then(|arr| get_fixed_bytes(arr, i)),
            d4: d4.and_then(|arr| get_fixed_bytes(arr, i)),
            d8: d8.and_then(|arr| get_fixed_bytes(arr, i)),
            a0: ax(i, 0)?,
            a1: ax(i, 1)?,
            a2: ax(i, 2)?,
            a3: ax(i, 3)?,
            a4: ax(i, 4)?,
            a5: ax(i, 5)?,
            a6: ax(i, 6)?,
            a7: ax(i, 7)?,
            a8: ax(i, 8)?,
            a9: ax(i, 9)?,
            is_inner: is_inner.and_then(|a| get_bool(a, i)),
            tx_success: tx_success.and_then(|a| get_bool(a, i)),
            error: error.and_then(|a| get_str(a, i)),
            compute_units_consumed: compute.and_then(|a| get_u64(a, i)),
        });
    }
    Ok(out)
}

pub fn logs_from_arrow(batch: &RecordBatch) -> Result<Vec<Log>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = opt_col::<UInt64Array>(batch, "slot")?;
    let tx_index = opt_col::<UInt32Array>(batch, "transaction_index")?;
    let inst_addr = opt_col::<ListArray>(batch, "instruction_address")?;
    let program_id = opt_col::<StringArray>(batch, "program_id")?;
    let kind = opt_col::<StringArray>(batch, "kind")?;
    let message = opt_col::<StringArray>(batch, "message")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(Log {
            slot: slot.and_then(|a| get_u64(a, i)),
            transaction_index: tx_index.and_then(|a| get_u32(a, i)),
            instruction_address: inst_addr
                .map(|a| get_u32_list(a, i, "logs.instruction_address"))
                .transpose()?
                .flatten(),
            program_id: program_id
                .map(|a| get_parsed::<Address>(a, i, "logs.program_id"))
                .transpose()?
                .flatten(),
            // Unknown kinds fold into Other (unknown-tolerant by design).
            kind: kind
                .and_then(|a| get_str(a, i))
                .map(|s| LogKind::from_stored(&s)),
            message: message.and_then(|a| get_str(a, i)),
        });
    }
    Ok(out)
}

/// Parse a served `token_state` value. The value set is closed and
/// server-controlled, so an unknown string is an error, not an Other.
fn parse_token_state(s: &str) -> Result<TokenState> {
    Ok(match s {
        "not_a_token" => TokenState::NotAToken,
        "opened" => TokenState::Opened,
        "closed" => TokenState::Closed,
        "persisted" => TokenState::Persisted,
        other => return Err(anyhow!("unknown token_state value {other:?}")),
    })
}

/// Decode the merged `account_activity` table.
pub fn account_activity_from_arrow(batch: &RecordBatch) -> Result<Vec<AccountActivity>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = opt_col::<UInt64Array>(batch, "slot")?;
    let tx_index = opt_col::<UInt32Array>(batch, "transaction_index")?;
    let transaction_id = opt_col::<StringArray>(batch, "transaction_id")?;
    let account_index = opt_col::<UInt32Array>(batch, "account_index")?;
    let account = opt_col::<StringArray>(batch, "account")?;
    let pre_balance = opt_col::<UInt64Array>(batch, "pre_balance")?;
    let post_balance = opt_col::<UInt64Array>(batch, "post_balance")?;
    let is_signer = opt_col::<BooleanArray>(batch, "is_signer")?;
    let is_writable = opt_col::<BooleanArray>(batch, "is_writable")?;
    let is_fee_payer = opt_col::<BooleanArray>(batch, "is_fee_payer")?;
    let from_lookup_table = opt_col::<BooleanArray>(batch, "from_lookup_table")?;
    let mint = opt_col::<StringArray>(batch, "mint")?;
    let pre_owner = opt_col::<StringArray>(batch, "pre_owner")?;
    let post_owner = opt_col::<StringArray>(batch, "post_owner")?;
    let token_decimals = opt_col::<UInt8Array>(batch, "token_decimals")?;
    let pre_token_balance = opt_col::<StringArray>(batch, "pre_token_balance")?;
    let post_token_balance = opt_col::<StringArray>(batch, "post_token_balance")?;
    let pre_program_id = opt_col::<StringArray>(batch, "pre_program_id")?;
    let post_program_id = opt_col::<StringArray>(batch, "post_program_id")?;
    let token_state = opt_col::<StringArray>(batch, "token_state")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(AccountActivity {
            slot: slot.and_then(|a| get_u64(a, i)),
            transaction_index: tx_index.and_then(|a| get_u32(a, i)),
            transaction_id: transaction_id
                .map(|a| get_parsed::<Signature>(a, i, "account_activity.transaction_id"))
                .transpose()?
                .flatten(),
            account_index: account_index.and_then(|a| get_u32(a, i)),
            account: account
                .map(|a| get_parsed::<Address>(a, i, "account_activity.account"))
                .transpose()?
                .flatten(),
            pre_balance: pre_balance.and_then(|a| get_u64(a, i)),
            post_balance: post_balance.and_then(|a| get_u64(a, i)),
            is_signer: is_signer.and_then(|a| get_bool(a, i)),
            is_writable: is_writable.and_then(|a| get_bool(a, i)),
            is_fee_payer: is_fee_payer.and_then(|a| get_bool(a, i)),
            from_lookup_table: from_lookup_table.and_then(|a| get_bool(a, i)),
            mint: mint
                .map(|a| get_parsed::<Address>(a, i, "account_activity.mint"))
                .transpose()?
                .flatten(),
            pre_owner: pre_owner
                .map(|a| get_parsed::<Address>(a, i, "account_activity.pre_owner"))
                .transpose()?
                .flatten(),
            post_owner: post_owner
                .map(|a| get_parsed::<Address>(a, i, "account_activity.post_owner"))
                .transpose()?
                .flatten(),
            token_decimals: token_decimals.and_then(|a| get_u8(a, i)),
            pre_token_balance: pre_token_balance
                .map(|a| get_u64_from_str(a, i, "account_activity.pre_token_balance"))
                .transpose()?
                .flatten(),
            post_token_balance: post_token_balance
                .map(|a| get_u64_from_str(a, i, "account_activity.post_token_balance"))
                .transpose()?
                .flatten(),
            pre_program_id: pre_program_id
                .map(|a| get_parsed::<Address>(a, i, "account_activity.pre_program_id"))
                .transpose()?
                .flatten(),
            post_program_id: post_program_id
                .map(|a| get_parsed::<Address>(a, i, "account_activity.post_program_id"))
                .transpose()?
                .flatten(),
            token_state: token_state
                .and_then(|a| get_str(a, i))
                .map(|s| parse_token_state(&s))
                .transpose()?,
        });
    }
    Ok(out)
}

pub fn rewards_from_arrow(batch: &RecordBatch) -> Result<Vec<Reward>> {
    let n = batch.num_rows();
    if n == 0 {
        return Ok(Vec::new());
    }
    let slot = opt_col::<UInt64Array>(batch, "slot")?;
    let pubkey = opt_col::<StringArray>(batch, "pubkey")?;
    let lamports = opt_col::<Int64Array>(batch, "lamports")?;
    let post_balance = opt_col::<UInt64Array>(batch, "post_balance")?;
    let reward_type = opt_col::<StringArray>(batch, "reward_type")?;
    let commission = opt_col::<UInt8Array>(batch, "commission")?;

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(Reward {
            slot: slot.and_then(|a| get_u64(a, i)),
            pubkey: pubkey
                .map(|a| get_parsed::<Address>(a, i, "rewards.pubkey"))
                .transpose()?
                .flatten(),
            lamports: lamports.and_then(|a| get_i64(a, i)),
            post_balance: post_balance.and_then(|a| get_u64(a, i)),
            reward_type: reward_type.and_then(|a| get_str(a, i)),
            commission: commission.and_then(|a| get_u8(a, i)),
        });
    }
    Ok(out)
}
