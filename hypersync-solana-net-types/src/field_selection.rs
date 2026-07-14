use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

/// Per-table field selection: which columns to include in the response.
/// If a table's field list is empty, all columns are returned.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolanaFieldSelection {
    #[serde(default)]
    pub block: Vec<BlockField>,
    #[serde(default)]
    pub transaction: Vec<TransactionField>,
    /// Renamed from `instruction`; legacy key accepted via serde alias.
    #[serde(default, alias = "instruction")]
    pub instruction_call: Vec<InstructionField>,
    #[serde(default)]
    pub log: Vec<LogField>,
    #[serde(default)]
    pub balance: Vec<BalanceField>,
    #[serde(default)]
    pub token_balance: Vec<TokenBalanceField>,
    #[serde(default)]
    pub reward: Vec<RewardField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum BlockField {
    Slot,
    Blockhash,
    ParentSlot,
    ParentBlockhash,
    BlockTime,
    BlockHeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TransactionField {
    Slot,
    TransactionIndex,
    /// `signatures[0]` (base58) — the canonical Solana transaction id.
    TransactionId,
    Signatures,
    FeePayer,
    Success,
    Err,
    Fee,
    ComputeUnitsConsumed,
    AccountKeys,
    RecentBlockhash,
    Version,
    LoadedAddressesWritable,
    LoadedAddressesReadonly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum InstructionField {
    Slot,
    TransactionIndex,
    InstructionAddress,
    ProgramId,
    Accounts,
    Data,
    D1,
    D2,
    D4,
    D8,
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    A8,
    A9,
    IsInner,
    IsCommitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum LogField {
    Slot,
    TransactionIndex,
    InstructionAddress,
    ProgramId,
    Kind,
    Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum BalanceField {
    Slot,
    TransactionIndex,
    Account,
    Pre,
    Post,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TokenBalanceField {
    Slot,
    TransactionIndex,
    Account,
    Mint,
    Owner,
    PreAmount,
    PostAmount,
    PreProgramId,
    PostProgramId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RewardField {
    Slot,
    Pubkey,
    Lamports,
    PostBalance,
    RewardType,
    Commission,
}
