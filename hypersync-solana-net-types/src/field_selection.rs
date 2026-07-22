use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString, VariantArray};

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

/// Fields that are computed at serving time rather than stored as parquet
/// columns. Excluded from [`SolanaFieldSelection::full_physical`]; every new
/// derived variant must be added here or the schema-coverage tests fail.
const DERIVED_TRANSACTION_FIELDS: &[TransactionField] = &[TransactionField::TransactionId];
const DERIVED_INSTRUCTION_FIELDS: &[InstructionField] = &[
    InstructionField::ExecutingAccountIndex,
    InstructionField::AccountIndexArguments,
];

impl SolanaFieldSelection {
    /// Selection naming every physical column of every table, in
    /// `hypersync-solana-schema` order: the full stored schema, nothing
    /// derived. This is the selection a replication client (e.g. a
    /// hypersync skar-pull follower) needs so no column is projected away.
    ///
    /// Derived wire fields ([`DERIVED_TRANSACTION_FIELDS`],
    /// [`DERIVED_INSTRUCTION_FIELDS`]) are excluded; the Wave 2 renamed
    /// fields (`ExecutingAccount`, `AccountArguments`) select the physical
    /// `program_id` / `accounts` columns. The variant-to-column mapping is
    /// locked to `hypersync-solana-schema` by this crate's tests, so a field
    /// added to an enum lands here automatically unless it is explicitly
    /// classified as derived.
    pub fn full_physical() -> Self {
        fn physical<T: strum::VariantArray + Copy + PartialEq>(derived: &[T]) -> Vec<T> {
            T::VARIANTS
                .iter()
                .copied()
                .filter(|v| !derived.contains(v))
                .collect()
        }
        Self {
            block: physical(&[]),
            transaction: physical(DERIVED_TRANSACTION_FIELDS),
            instruction_call: physical(DERIVED_INSTRUCTION_FIELDS),
            log: physical(&[]),
            balance: physical(&[]),
            token_balance: physical(&[]),
            reward: physical(&[]),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    VariantArray,
)]
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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    VariantArray,
)]
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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    VariantArray,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum InstructionField {
    Slot,
    TransactionIndex,
    InstructionAddress,
    /// The invoked program's account. Renamed from `program_id`; the legacy
    /// wire name is still accepted on input via a serde alias.
    #[serde(alias = "program_id")]
    ExecutingAccount,
    /// Index of the executing account within the transaction's account keys.
    ExecutingAccountIndex,
    /// The instruction's account arguments (pubkeys). Renamed from `accounts`;
    /// the legacy wire name is still accepted on input via a serde alias.
    #[serde(alias = "accounts")]
    AccountArguments,
    /// Indexes of the account arguments within the transaction's account keys.
    AccountIndexArguments,
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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    VariantArray,
)]
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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    VariantArray,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum BalanceField {
    Slot,
    TransactionIndex,
    Account,
    Pre,
    Post,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    VariantArray,
)]
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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    VariantArray,
)]
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

/// Lock `full_physical` and the derived-field classification to the parquet
/// schemas in `hypersync-solana-schema`. Every enum variant must either map
/// to a physical column (and appear in `full_physical` at the column's
/// schema position) or be listed in a `DERIVED_*` const: adding a field
/// without classifying it fails here.
#[cfg(test)]
mod schema_coverage {
    use std::fmt::Display;

    use strum::VariantArray;

    use super::*;

    /// Wire name -> physical column name for the Wave 2 renames. Everything
    /// else matches by its snake_case name.
    fn physical_name(wire: String) -> String {
        match wire.as_str() {
            "executing_account" => "program_id".to_owned(),
            "account_arguments" => "accounts".to_owned(),
            other => other.to_owned(),
        }
    }

    fn column_names(schema: arrow::datatypes::SchemaRef) -> Vec<String> {
        schema.fields().iter().map(|f| f.name().clone()).collect()
    }

    fn assert_table<T>(table: &str, selected: &[T], schema: arrow::datatypes::SchemaRef)
    where
        T: VariantArray + Display + Copy + PartialEq,
    {
        let columns = column_names(schema);
        let mapped: Vec<String> = selected
            .iter()
            .map(|f| physical_name(f.to_string()))
            .collect();
        assert_eq!(
            mapped, columns,
            "{table}: full_physical must name every physical column in schema order"
        );
        // A variant excluded from full_physical is derived; its wire name
        // must not shadow a physical column (catches "was derived, became
        // physical" without updating the classification).
        for v in T::VARIANTS {
            if !selected.contains(v) {
                let wire = physical_name(v.to_string());
                assert!(
                    !columns.contains(&wire),
                    "{table}: variant `{v}` is classified derived but `{wire}` is a physical column"
                );
            }
        }
    }

    #[test]
    fn full_physical_matches_schemas() {
        let sel = SolanaFieldSelection::full_physical();
        assert_table("block", &sel.block, hypersync_solana_schema::block());
        assert_table(
            "transaction",
            &sel.transaction,
            hypersync_solana_schema::transaction(),
        );
        assert_table(
            "instruction",
            &sel.instruction_call,
            hypersync_solana_schema::instruction(),
        );
        assert_table("log", &sel.log, hypersync_solana_schema::log());
        assert_table("balance", &sel.balance, hypersync_solana_schema::balance());
        assert_table(
            "token_balance",
            &sel.token_balance,
            hypersync_solana_schema::token_balance(),
        );
        assert_table("reward", &sel.reward, hypersync_solana_schema::reward());
    }
}
