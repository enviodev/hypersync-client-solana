use std::str::FromStr;

use anyhow::{Context, Result};
use hypersync_solana_net_types::field_selection::{
    AccountActivityField, BlockField, InstructionField, LogField, RewardField,
    SolanaFieldSelection, TransactionField,
};
use hypersync_solana_net_types::query::{
    AccountActivitySelection as RsAccountActivitySelection,
    InstructionSelection as RsInstructionSelection, LogSelection as RsLogSelection,
    SolanaQuery as RsSolanaQuery, TransactionSelection as RsTransactionSelection,
};

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

/// Filter for selecting instructions. All non-empty fields are AND-ed.
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
    /// Commit status of the parent transaction. None: match both committed and
    /// failed. true: successful transactions only. false: failed only.
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
    pub kind: Option<Vec<String>>,
}

/// Filter for selecting rows of the merged `account_activity` table. All
/// non-empty fields are AND-ed. Because the table carries the native SOL and
/// SPL token sides on one row, this replaces pairing a `BalanceSelection` with
/// a `TokenBalanceSelection`.
#[napi(object)]
#[derive(Default, Clone)]
pub struct AccountActivitySelection {
    pub account: Option<Vec<String>>,
    pub mint: Option<Vec<String>>,
    pub owner: Option<Vec<String>>,
    pub program_id: Option<Vec<String>>,
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
    /// Return merged account activity for the matched result set without
    /// requiring `include_all_blocks`.
    pub include_account_activity: Option<bool>,
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

impl From<InstructionSelection> for RsInstructionSelection {
    fn from(s: InstructionSelection) -> Self {
        RsInstructionSelection {
            executing_account: s.executing_account.or(s.program_id).unwrap_or_default(),
            d1: s.d1.unwrap_or_default(),
            d2: s.d2.unwrap_or_default(),
            d4: s.d4.unwrap_or_default(),
            d8: s.d8.unwrap_or_default(),
            a0: s.a0.unwrap_or_default(),
            a1: s.a1.unwrap_or_default(),
            a2: s.a2.unwrap_or_default(),
            a3: s.a3.unwrap_or_default(),
            a4: s.a4.unwrap_or_default(),
            a5: s.a5.unwrap_or_default(),
            a6: s.a6.unwrap_or_default(),
            a7: s.a7.unwrap_or_default(),
            a8: s.a8.unwrap_or_default(),
            a9: s.a9.unwrap_or_default(),
            is_inner: s.is_inner,
            is_committed: s.is_committed,
        }
    }
}

impl TryFrom<TransactionSelection> for RsTransactionSelection {
    type Error = anyhow::Error;

    fn try_from(s: TransactionSelection) -> Result<Self> {
        Ok(RsTransactionSelection {
            fee_payer: s.fee_payer.unwrap_or_default(),
            transaction_id: s.transaction_id.unwrap_or_default(),
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

impl From<LogSelection> for RsLogSelection {
    fn from(s: LogSelection) -> Self {
        RsLogSelection {
            program_id: s.program_id.unwrap_or_default(),
            kind: s.kind.unwrap_or_default(),
        }
    }
}

impl From<AccountActivitySelection> for RsAccountActivitySelection {
    fn from(s: AccountActivitySelection) -> Self {
        RsAccountActivitySelection {
            account: s.account.unwrap_or_default(),
            mint: s.mint.unwrap_or_default(),
            owner: s.owner.unwrap_or_default(),
            program_id: s.program_id.unwrap_or_default(),
        }
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
                .map(Into::into)
                .collect(),
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
                .map(Into::into)
                .collect(),
            account_activity: q
                .account_activity
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            include_all_blocks: q.include_all_blocks.unwrap_or_default(),
            include_account_activity: q.include_account_activity.unwrap_or_default(),
            field_selection,
            max_num_blocks: q.max_num_blocks.map(|v| v.max(0) as usize),
            max_num_transactions: q.max_num_transactions.map(|v| v.max(0) as usize),
            max_num_instructions: q.max_num_instructions.map(|v| v.max(0) as usize),
            max_num_logs: q.max_num_logs.map(|v| v.max(0) as usize),
            max_num_account_activity,
        })
    }
}
