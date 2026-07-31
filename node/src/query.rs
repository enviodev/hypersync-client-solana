use std::str::FromStr;

use anyhow::{Context, Result};
use hypersync_solana_net_types::field_selection::{
    AccountActivityField, BlockField, InstructionField, LogField, RewardField,
    SolanaFieldSelection, TransactionField,
};
use hypersync_solana_net_types::query::{
    AccountActivitySelection as RsAccountActivitySelection, ActivityKind as RsActivityKind,
    InstructionSelection as RsInstructionSelection, LogSelection as RsLogSelection,
    SolanaQuery as RsSolanaQuery, TransactionSelection as RsTransactionSelection,
};
use hypersync_solana_net_types::{Address, LogKind, Signature};

/// Per-table field selection. Each list is a set of column names in `snake_case`.
/// An empty (or missing) list means "return all columns for that table".
#[napi(object)]
#[derive(Default, Clone)]
pub struct FieldSelection {
    pub block: Option<Vec<String>>,
    pub transaction: Option<Vec<String>>,
    pub instruction_call: Option<Vec<String>>,
    /// @deprecated renamed to `instructionCall`; still honored when
    /// `instructionCall` is absent.
    pub instruction: Option<Vec<String>>,
    pub log: Option<Vec<String>>,
    pub account_activity: Option<Vec<String>>,
    pub reward: Option<Vec<String>>,
}

/// Filter for selecting instruction calls. All non-empty fields are AND-ed.
#[napi(object)]
#[derive(Default, Clone)]
pub struct InstructionSelection {
    pub executing_account: Option<Vec<String>>,
    /// @deprecated renamed to `executingAccount`; still honored when
    /// `executingAccount` is absent.
    pub program_id: Option<Vec<String>>,
    pub d1: Option<Vec<String>>,
    pub d2: Option<Vec<String>>,
    pub d4: Option<Vec<String>>,
    pub d8: Option<Vec<String>>,
    pub a0: Option<Vec<String>>,
    pub a1: Option<Vec<String>>,
    pub a2: Option<Vec<String>>,
    pub a3: Option<Vec<String>>,
    pub a4: Option<Vec<String>>,
    pub a5: Option<Vec<String>>,
    pub a6: Option<Vec<String>>,
    pub a7: Option<Vec<String>>,
    pub a8: Option<Vec<String>>,
    pub a9: Option<Vec<String>>,
    /// None: match both outer and inner. true: inner only. false: outer only.
    pub is_inner: Option<bool>,
    /// Success of the PARENT transaction. None: match instructions of both
    /// successful and failed transactions. true: successful only. false:
    /// failed only. Instructions of failed transactions had their state
    /// changes rolled back, so consumers that count effects should set true.
    pub tx_success: Option<bool>,
    /// @deprecated renamed to `txSuccess`; still honored when `txSuccess` is
    /// absent.
    pub is_committed: Option<bool>,
}

/// Filter for selecting transactions. All non-empty fields are AND-ed.
#[napi(object)]
#[derive(Default, Clone)]
pub struct TransactionSelection {
    pub fee_payer: Option<Vec<String>>,
    /// Base58 `signatures[0]`, the canonical Solana transaction signature.
    pub transaction_id: Option<Vec<String>>,
    /// Position of the transaction within its block.
    pub transaction_index: Option<Vec<i64>>,
    pub success: Option<bool>,
}

/// Filter for selecting logs. All non-empty fields are AND-ed.
#[napi(object)]
#[derive(Default, Clone)]
pub struct LogSelection {
    pub program_id: Option<Vec<String>>,
    /// Log kinds to match: invoke/success/failed/consumed/log/data/other.
    /// SQD-ingested and default RPC-ingested ranges only carry
    /// log/data/other rows.
    pub kind: Option<Vec<String>>,
}

/// Filter for selecting rows of the merged `account_activity` table. All
/// non-empty fields are AND-ed. Because the table carries the native SOL and
/// SPL token sides on one row, this replaces pairing a `BalanceSelection` with
/// a `TokenBalanceSelection`.
#[napi(object)]
#[derive(Default, Clone)]
pub struct AccountActivitySelection {
    /// Restrict to rows carrying a given side of the merge: "native",
    /// "token", or both. A row carrying both sides matches either value, so
    /// `["native"]` is the row set the removed `balances` table held.
    pub kind: Option<Vec<String>>,
    pub account: Option<Vec<String>>,
    pub transaction_id: Option<Vec<String>>,
    pub mint: Option<Vec<String>>,
    /// Matches either the pre or the post owner (the stored column is split
    /// so an in-transaction owner change stays visible).
    pub owner: Option<Vec<String>>,
    pub program_id: Option<Vec<String>>,
    /// Position flags. A row whose flag is null (the source could not derive
    /// it) matches neither true nor false.
    pub is_signer: Option<bool>,
    pub is_writable: Option<bool>,
    pub is_fee_payer: Option<bool>,
    pub from_lookup_table: Option<bool>,
}

/// Top-level Solana HyperSync query. Returns block bundles matching the
/// given filters within `[from_slot, to_slot)`.
#[napi(object)]
#[derive(Default, Clone)]
pub struct SolanaQuery {
    /// Inclusive start slot.
    pub from_slot: i64,
    /// Exclusive end slot. If omitted, queries run to the current height.
    pub to_slot: Option<i64>,
    pub instruction_calls: Option<Vec<InstructionSelection>>,
    /// @deprecated renamed to `instructionCalls`; still honored when
    /// `instructionCalls` is absent.
    pub instructions: Option<Vec<InstructionSelection>>,
    pub transactions: Option<Vec<TransactionSelection>>,
    pub logs: Option<Vec<LogSelection>>,
    pub account_activity: Option<Vec<AccountActivitySelection>>,
    pub include_all_blocks: Option<bool>,
    /// Per-table field selection (which columns to return).
    pub field_selection: Option<FieldSelection>,
    pub max_num_blocks: Option<i64>,
    pub max_num_transactions: Option<i64>,
    pub max_num_instructions: Option<i64>,
    pub max_num_logs: Option<i64>,
    pub max_num_account_activity: Option<i64>,
}

fn parse_enum_list<T: FromStr>(name: &str, vals: Vec<String>) -> Result<Vec<T>>
where
    T::Err: std::fmt::Display,
{
    vals.into_iter()
        .map(|s| {
            T::from_str(&s).map_err(|e| anyhow::anyhow!("invalid {} field '{}': {}", name, s, e))
        })
        .collect()
}

/// Parse a list of base58 strings into a typed value list (Address /
/// Signature / LogKind). Malformed entries error fail-fast with the filter
/// field named, rather than silently matching nothing.
fn parse_list<T: FromStr>(name: &str, vals: Option<Vec<String>>) -> Result<Vec<T>>
where
    T::Err: std::fmt::Display,
{
    vals.unwrap_or_default()
        .into_iter()
        .map(|s| {
            T::from_str(&s).map_err(|e| anyhow::anyhow!("invalid {} value '{}': {}", name, s, e))
        })
        .collect()
}

impl TryFrom<FieldSelection> for SolanaFieldSelection {
    type Error = anyhow::Error;

    fn try_from(f: FieldSelection) -> Result<Self> {
        Ok(SolanaFieldSelection {
            block: parse_enum_list::<BlockField>("block", f.block.unwrap_or_default())?,
            transaction: parse_enum_list::<TransactionField>(
                "transaction",
                f.transaction.unwrap_or_default(),
            )?,
            instruction_call: parse_enum_list::<InstructionField>(
                "instruction_call",
                f.instruction_call.or(f.instruction).unwrap_or_default(),
            )?,
            log: parse_enum_list::<LogField>("log", f.log.unwrap_or_default())?,
            account_activity: parse_enum_list::<AccountActivityField>(
                "account_activity",
                f.account_activity.unwrap_or_default(),
            )?,
            reward: parse_enum_list::<RewardField>("reward", f.reward.unwrap_or_default())?,
        })
    }
}

impl TryFrom<InstructionSelection> for RsInstructionSelection {
    type Error = anyhow::Error;

    fn try_from(s: InstructionSelection) -> Result<Self> {
        Ok(RsInstructionSelection {
            executing_account: parse_list::<Address>(
                "executing_account",
                s.executing_account.or(s.program_id),
            )?,
            d1: s.d1.unwrap_or_default(),
            d2: s.d2.unwrap_or_default(),
            d4: s.d4.unwrap_or_default(),
            d8: s.d8.unwrap_or_default(),
            a0: parse_list::<Address>("a0", s.a0)?,
            a1: parse_list::<Address>("a1", s.a1)?,
            a2: parse_list::<Address>("a2", s.a2)?,
            a3: parse_list::<Address>("a3", s.a3)?,
            a4: parse_list::<Address>("a4", s.a4)?,
            a5: parse_list::<Address>("a5", s.a5)?,
            a6: parse_list::<Address>("a6", s.a6)?,
            a7: parse_list::<Address>("a7", s.a7)?,
            a8: parse_list::<Address>("a8", s.a8)?,
            a9: parse_list::<Address>("a9", s.a9)?,
            is_inner: s.is_inner,
            tx_success: s.tx_success.or(s.is_committed),
        })
    }
}

impl TryFrom<TransactionSelection> for RsTransactionSelection {
    type Error = anyhow::Error;

    fn try_from(s: TransactionSelection) -> Result<Self> {
        Ok(RsTransactionSelection {
            fee_payer: parse_list::<Address>("fee_payer", s.fee_payer)?,
            transaction_id: parse_list::<Signature>("transaction_id", s.transaction_id)?,
            transaction_index: s
                .transaction_index
                .unwrap_or_default()
                .into_iter()
                .map(|v| u64::try_from(v).context("transaction_index must be non-negative"))
                .collect::<Result<Vec<_>>>()?,
            success: s.success,
        })
    }
}

impl TryFrom<LogSelection> for RsLogSelection {
    type Error = anyhow::Error;

    fn try_from(s: LogSelection) -> Result<Self> {
        Ok(RsLogSelection {
            program_id: parse_list::<Address>("program_id", s.program_id)?,
            kind: parse_list::<LogKind>("kind", s.kind)?,
        })
    }
}

impl TryFrom<AccountActivitySelection> for RsAccountActivitySelection {
    type Error = anyhow::Error;

    fn try_from(s: AccountActivitySelection) -> Result<Self> {
        let kind = s
            .kind
            .unwrap_or_default()
            .into_iter()
            .map(|k| match k.as_str() {
                "native" => Ok(RsActivityKind::Native),
                "token" => Ok(RsActivityKind::Token),
                other => Err(anyhow::anyhow!(
                    "unknown account activity kind `{other}`, expected \"native\" or \"token\""
                )),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(RsAccountActivitySelection {
            kind,
            account: parse_list::<Address>("account", s.account)?,
            transaction_id: parse_list::<Signature>("transaction_id", s.transaction_id)?,
            mint: parse_list::<Address>("mint", s.mint)?,
            owner: parse_list::<Address>("owner", s.owner)?,
            program_id: parse_list::<Address>("program_id", s.program_id)?,
            is_signer: s.is_signer,
            is_writable: s.is_writable,
            is_fee_payer: s.is_fee_payer,
            from_lookup_table: s.from_lookup_table,
        })
    }
}

impl TryFrom<SolanaQuery> for RsSolanaQuery {
    type Error = anyhow::Error;

    fn try_from(q: SolanaQuery) -> Result<Self> {
        let from_slot = u64::try_from(q.from_slot).context("from_slot must be non-negative")?;
        let to_slot = q
            .to_slot
            .map(|v| u64::try_from(v).context("to_slot must be non-negative"))
            .transpose()?;
        let field_selection = q
            .field_selection
            .map(SolanaFieldSelection::try_from)
            .transpose()?
            .unwrap_or_default();
        // Validate the account-activity limit fail-fast (like from_slot /
        // to_slot) rather than silently clamping a negative caller bug to 0.
        let max_num_account_activity = q
            .max_num_account_activity
            .map(|v| usize::try_from(v).context("max_num_account_activity must be non-negative"))
            .transpose()?;

        Ok(RsSolanaQuery {
            from_slot,
            to_slot,
            instruction_calls: q
                .instruction_calls
                .or(q.instructions)
                .unwrap_or_default()
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>>>()?,
            transactions: q
                .transactions
                .unwrap_or_default()
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>>>()?,
            logs: q
                .logs
                .unwrap_or_default()
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>>>()?,
            account_activity: q
                .account_activity
                .unwrap_or_default()
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>>>()?,
            include_all_blocks: q.include_all_blocks.unwrap_or_default(),
            field_selection,
            max_num_blocks: q.max_num_blocks.map(|v| v.max(0) as usize),
            max_num_transactions: q.max_num_transactions.map(|v| v.max(0) as usize),
            max_num_instructions: q.max_num_instructions.map(|v| v.max(0) as usize),
            max_num_logs: q.max_num_logs.map(|v| v.max(0) as usize),
            max_num_account_activity,
        })
    }
}
