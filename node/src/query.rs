use std::str::FromStr;

use anyhow::{Context, Result};
use hypersync_solana_net_types::field_selection::{
    BalanceField, BlockField, InstructionField, LogField, RewardField, SolanaFieldSelection,
    TokenBalanceField, TransactionField,
};
use hypersync_solana_net_types::query::{
    BalanceSelection as RsBalanceSelection, InstructionSelection as RsInstructionSelection,
    LogSelection as RsLogSelection, SolanaQuery as RsSolanaQuery,
    TokenBalanceSelection as RsTokenBalanceSelection,
    TransactionSelection as RsTransactionSelection,
};

/// Per-table field selection. Each list is a set of column names in `snake_case`.
/// An empty (or missing) list means "return all columns for that table".
#[napi(object)]
#[derive(Default, Clone)]
pub struct FieldSelection {
    pub block: Option<Vec<String>>,
    pub transaction: Option<Vec<String>>,
    pub instruction: Option<Vec<String>>,
    pub log: Option<Vec<String>>,
    pub balance: Option<Vec<String>>,
    pub token_balance: Option<Vec<String>>,
    pub reward: Option<Vec<String>>,
}

/// Filter for selecting instructions. All non-empty fields are AND-ed.
#[napi(object)]
#[derive(Default, Clone)]
pub struct InstructionSelection {
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
    pub include_transaction: Option<bool>,
    pub include_logs: Option<bool>,
    pub include_inner_instructions: Option<bool>,
    /// Also return native SOL balances for matched txs (scoped join).
    pub include_balances: Option<bool>,
    /// Also return SPL token balances for matched txs (scoped join).
    pub include_token_balances: Option<bool>,
}

/// Filter for selecting transactions. All non-empty fields are AND-ed.
#[napi(object)]
#[derive(Default, Clone)]
pub struct TransactionSelection {
    pub fee_payer: Option<Vec<String>>,
    pub success: Option<bool>,
    pub include_instructions: Option<bool>,
    /// Also return native SOL balances for matched txs (scoped join).
    pub include_balances: Option<bool>,
    /// Also return SPL token balances for matched txs (scoped join).
    pub include_token_balances: Option<bool>,
}

/// Filter for selecting logs. All non-empty fields are AND-ed.
#[napi(object)]
#[derive(Default, Clone)]
pub struct LogSelection {
    pub program_id: Option<Vec<String>>,
    pub kind: Option<Vec<String>>,
    pub include_transaction: Option<bool>,
    pub include_instruction: Option<bool>,
    /// Also return native SOL balances for matched txs (scoped join).
    pub include_balances: Option<bool>,
    /// Also return SPL token balances for matched txs (scoped join).
    pub include_token_balances: Option<bool>,
}

/// Filter for selecting native SOL balance changes. All non-empty fields are AND-ed.
#[napi(object)]
#[derive(Default, Clone)]
pub struct BalanceSelection {
    pub account: Option<Vec<String>>,
}

/// Filter for selecting SPL token balance changes. All non-empty fields are AND-ed.
#[napi(object)]
#[derive(Default, Clone)]
pub struct TokenBalanceSelection {
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
    pub instructions: Option<Vec<InstructionSelection>>,
    pub transactions: Option<Vec<TransactionSelection>>,
    pub logs: Option<Vec<LogSelection>>,
    pub balances: Option<Vec<BalanceSelection>>,
    pub token_balances: Option<Vec<TokenBalanceSelection>>,
    pub include_all_blocks: Option<bool>,
    /// Return native SOL balances for the matched result set without requiring
    /// `include_all_blocks`.
    pub include_balances: Option<bool>,
    /// Return SPL token balances for the matched result set without requiring
    /// `include_all_blocks`.
    pub include_token_balances: Option<bool>,
    pub fields: Option<FieldSelection>,
    pub max_num_blocks: Option<i64>,
    pub max_num_transactions: Option<i64>,
    pub max_num_instructions: Option<i64>,
    pub max_num_logs: Option<i64>,
    pub max_num_balances: Option<i64>,
    pub max_num_token_balances: Option<i64>,
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
            instruction: parse_enum_list::<InstructionField>(
                "instruction",
                f.instruction.unwrap_or_default(),
            )?,
            log: parse_enum_list::<LogField>("log", f.log.unwrap_or_default())?,
            balance: parse_enum_list::<BalanceField>("balance", f.balance.unwrap_or_default())?,
            token_balance: parse_enum_list::<TokenBalanceField>(
                "token_balance",
                f.token_balance.unwrap_or_default(),
            )?,
            reward: parse_enum_list::<RewardField>("reward", f.reward.unwrap_or_default())?,
        })
    }
}

impl From<InstructionSelection> for RsInstructionSelection {
    fn from(s: InstructionSelection) -> Self {
        RsInstructionSelection {
            program_id: s.program_id.unwrap_or_default(),
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
            include_transaction: s.include_transaction.unwrap_or_default(),
            include_logs: s.include_logs.unwrap_or_default(),
            include_inner_instructions: s.include_inner_instructions.unwrap_or_default(),
            include_balances: s.include_balances.unwrap_or_default(),
            include_token_balances: s.include_token_balances.unwrap_or_default(),
        }
    }
}

impl From<TransactionSelection> for RsTransactionSelection {
    fn from(s: TransactionSelection) -> Self {
        RsTransactionSelection {
            fee_payer: s.fee_payer.unwrap_or_default(),
            success: s.success,
            include_instructions: s.include_instructions.unwrap_or_default(),
            include_balances: s.include_balances.unwrap_or_default(),
            include_token_balances: s.include_token_balances.unwrap_or_default(),
        }
    }
}

impl From<LogSelection> for RsLogSelection {
    fn from(s: LogSelection) -> Self {
        RsLogSelection {
            program_id: s.program_id.unwrap_or_default(),
            kind: s.kind.unwrap_or_default(),
            include_transaction: s.include_transaction.unwrap_or_default(),
            include_instruction: s.include_instruction.unwrap_or_default(),
            include_balances: s.include_balances.unwrap_or_default(),
            include_token_balances: s.include_token_balances.unwrap_or_default(),
        }
    }
}

impl From<BalanceSelection> for RsBalanceSelection {
    fn from(s: BalanceSelection) -> Self {
        RsBalanceSelection {
            account: s.account.unwrap_or_default(),
        }
    }
}

impl From<TokenBalanceSelection> for RsTokenBalanceSelection {
    fn from(s: TokenBalanceSelection) -> Self {
        RsTokenBalanceSelection {
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
        let fields = q
            .fields
            .map(SolanaFieldSelection::try_from)
            .transpose()?
            .unwrap_or_default();
        // Validate the new balance limits fail-fast (like from_slot / to_slot)
        // rather than silently clamping a negative caller bug to 0.
        let max_num_balances = q
            .max_num_balances
            .map(|v| usize::try_from(v).context("max_num_balances must be non-negative"))
            .transpose()?;
        let max_num_token_balances = q
            .max_num_token_balances
            .map(|v| usize::try_from(v).context("max_num_token_balances must be non-negative"))
            .transpose()?;

        Ok(RsSolanaQuery {
            from_slot,
            to_slot,
            instructions: q
                .instructions
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            transactions: q
                .transactions
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            logs: q
                .logs
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            balances: q
                .balances
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            token_balances: q
                .token_balances
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            include_all_blocks: q.include_all_blocks.unwrap_or_default(),
            include_balances: q.include_balances.unwrap_or_default(),
            include_token_balances: q.include_token_balances.unwrap_or_default(),
            fields,
            max_num_blocks: q.max_num_blocks.map(|v| v.max(0) as usize),
            max_num_transactions: q.max_num_transactions.map(|v| v.max(0) as usize),
            max_num_instructions: q.max_num_instructions.map(|v| v.max(0) as usize),
            max_num_logs: q.max_num_logs.map(|v| v.max(0) as usize),
            max_num_balances,
            max_num_token_balances,
        })
    }
}
