use serde::{Deserialize, Serialize};

/// Top-level Solana HyperSync query.
///
/// Returns block bundles matching the given filters within [from_slot, to_slot).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolanaQuery {
    /// Inclusive start slot.
    pub from_slot: u64,
    /// Exclusive end slot. If omitted, query runs to the current height.
    #[serde(default)]
    pub to_slot: Option<u64>,
    /// Instruction-level selections. A block is included if any instruction
    /// in any of its transactions matches at least one selection.
    #[serde(default)]
    pub instructions: Vec<InstructionSelection>,
    /// Transaction-level selections. A block is included if any transaction
    /// matches at least one selection.
    #[serde(default)]
    pub transactions: Vec<TransactionSelection>,
    /// If true, return all blocks in the range regardless of filter matches.
    #[serde(default)]
    pub include_all_blocks: bool,
    /// Per-table field selection (which columns to return).
    ///
    /// Named `field_selection` for consistency with the EVM and Fuel HyperSync
    /// query APIs.
    #[serde(default)]
    pub field_selection: crate::field_selection::SolanaFieldSelection,
    /// Maximum number of instructions to return before stopping.
    #[serde(default)]
    pub max_num_instructions: Option<usize>,
    /// Maximum number of transactions to return before stopping.
    #[serde(default)]
    pub max_num_transactions: Option<usize>,
    /// Log-level selections. A log is included if it matches at least one selection.
    #[serde(default)]
    pub logs: Vec<LogSelection>,
    /// Maximum number of blocks to return before stopping.
    #[serde(default)]
    pub max_num_blocks: Option<usize>,
    /// Maximum number of logs to return before stopping.
    #[serde(default)]
    pub max_num_logs: Option<usize>,

    /// Native SOL balance selections. A balance row is included if it matches
    /// at least one selection (empty selection `{}` matches all). Balances are
    /// keyed to a transaction via `transaction_index`.
    ///
    /// Unlike `include_all_blocks`, requesting balances this way does NOT force
    /// every block in the range to be returned.
    #[serde(default)]
    pub balances: Vec<BalanceSelection>,
    /// SPL token balance selections. Same semantics as `balances`.
    #[serde(default)]
    pub token_balances: Vec<TokenBalanceSelection>,
    /// When true, return native SOL `balances` for the matched result set
    /// without requiring `include_all_blocks`. With no other filters this
    /// returns all balances in range (SQD parity); combined with a per-selection
    /// scoped join it returns only the balances for matched transactions.
    #[serde(default)]
    pub include_balances: bool,
    /// When true, return SPL `token_balances` for the matched result set without
    /// requiring `include_all_blocks`. See `include_balances`.
    #[serde(default)]
    pub include_token_balances: bool,
    /// Maximum number of balance rows to return before stopping.
    #[serde(default)]
    pub max_num_balances: Option<usize>,
    /// Maximum number of token balance rows to return before stopping.
    #[serde(default)]
    pub max_num_token_balances: Option<usize>,
}

/// Filter for selecting instructions.
///
/// All non-empty fields are AND-ed: an instruction must match at least one value
/// in every non-empty field. Empty fields are ignored (match-all).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstructionSelection {
    /// Match instructions whose program_id is one of these pubkeys.
    #[serde(default)]
    pub program_id: Vec<String>,

    /// Match first 1 byte of instruction data (hex-encoded, e.g. "e8").
    #[serde(default)]
    pub d1: Vec<String>,
    /// Match first 2 bytes of instruction data (hex-encoded).
    #[serde(default)]
    pub d2: Vec<String>,
    /// Match first 4 bytes of instruction data (hex-encoded).
    #[serde(default)]
    pub d4: Vec<String>,
    /// Match first 8 bytes of instruction data (hex-encoded, Anchor discriminator).
    #[serde(default)]
    pub d8: Vec<String>,

    /// Match account at position 0.
    #[serde(default)]
    pub a0: Vec<String>,
    /// Match account at position 1.
    #[serde(default)]
    pub a1: Vec<String>,
    /// Match account at position 2.
    #[serde(default)]
    pub a2: Vec<String>,
    /// Match account at position 3.
    #[serde(default)]
    pub a3: Vec<String>,
    /// Match account at position 4.
    #[serde(default)]
    pub a4: Vec<String>,
    /// Match account at position 5.
    #[serde(default)]
    pub a5: Vec<String>,
    /// Match account at position 6.
    #[serde(default)]
    pub a6: Vec<String>,
    /// Match account at position 7.
    #[serde(default)]
    pub a7: Vec<String>,
    /// Match account at position 8.
    #[serde(default)]
    pub a8: Vec<String>,
    /// Match account at position 9.
    #[serde(default)]
    pub a9: Vec<String>,

    /// Filter on inner-instruction status:
    /// - None / absent: match both outer and inner
    /// - Some(true): only inner instructions
    /// - Some(false): only outer instructions
    #[serde(default)]
    pub is_inner: Option<bool>,
}

impl InstructionSelection {
    pub fn is_empty(&self) -> bool {
        self.program_id.is_empty()
            && self.d1.is_empty()
            && self.d2.is_empty()
            && self.d4.is_empty()
            && self.d8.is_empty()
            && self.a0.is_empty()
            && self.a1.is_empty()
            && self.a2.is_empty()
            && self.a3.is_empty()
            && self.a4.is_empty()
            && self.a5.is_empty()
            && self.a6.is_empty()
            && self.a7.is_empty()
            && self.a8.is_empty()
            && self.a9.is_empty()
            && self.is_inner.is_none()
    }
}

/// Filter for selecting transactions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionSelection {
    /// Match transactions whose fee_payer is one of these pubkeys.
    #[serde(default)]
    pub fee_payer: Vec<String>,
    /// Match transactions by transaction id (`signatures[0]`, base58). This is the
    /// canonical Solana transaction signature and acts as the transaction's id.
    #[serde(default)]
    pub transaction_id: Vec<String>,
    /// Match transactions by their `transaction_index` (position within the block).
    #[serde(default)]
    pub transaction_index: Vec<u64>,
    /// If set, only match transactions with this success status.
    #[serde(default)]
    pub success: Option<bool>,
}

impl TransactionSelection {
    pub fn is_empty(&self) -> bool {
        self.fee_payer.is_empty()
            && self.transaction_id.is_empty()
            && self.transaction_index.is_empty()
            && self.success.is_none()
    }
}

/// Filter for selecting logs.
///
/// All non-empty fields are AND-ed: a log must match at least one value
/// in every non-empty field. Empty fields are ignored (match-all).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogSelection {
    /// Match logs whose program_id is one of these pubkeys.
    #[serde(default)]
    pub program_id: Vec<String>,
    /// Match logs whose kind is one of these values (e.g. "log", "data").
    #[serde(default)]
    pub kind: Vec<String>,
}

impl LogSelection {
    pub fn is_empty(&self) -> bool {
        self.program_id.is_empty() && self.kind.is_empty()
    }
}

/// Filter for selecting native SOL balance changes.
///
/// All non-empty fields are AND-ed: a balance must match at least one value in
/// every non-empty field. Empty fields are ignored (match-all). An empty
/// selection `{}` returns every balance row in the queried range.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BalanceSelection {
    /// Match balances whose account is one of these pubkeys.
    #[serde(default)]
    pub account: Vec<String>,
}

impl BalanceSelection {
    pub fn is_empty(&self) -> bool {
        self.account.is_empty()
    }
}

/// Filter for selecting SPL token balance changes.
///
/// All non-empty fields are AND-ed: a token balance must match at least one
/// value in every non-empty field. Empty fields are ignored (match-all). An
/// empty selection `{}` returns every token balance row in the queried range.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenBalanceSelection {
    /// Match by token account address (the ATA / raw token account).
    #[serde(default)]
    pub account: Vec<String>,
    /// Match by mint address.
    #[serde(default)]
    pub mint: Vec<String>,
    /// Match by owner (wallet) address.
    #[serde(default)]
    pub owner: Vec<String>,
    /// Match by token program id (e.g. classic SPL Token vs Token-2022).
    /// Matches the post program id, falling back to the pre program id.
    #[serde(default)]
    pub program_id: Vec<String>,
}

impl TokenBalanceSelection {
    pub fn is_empty(&self) -> bool {
        self.account.is_empty()
            && self.mint.is_empty()
            && self.owner.is_empty()
            && self.program_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_selection::BlockField;

    #[test]
    fn field_selection_canonical_key_deserializes() {
        let q: SolanaQuery =
            serde_json::from_str(r#"{"from_slot":0,"field_selection":{"block":["slot"]}}"#)
                .unwrap();
        assert_eq!(q.field_selection.block, vec![BlockField::Slot]);
    }

    #[test]
    fn field_selection_serializes_with_new_key() {
        let q = SolanaQuery {
            field_selection: crate::field_selection::SolanaFieldSelection {
                block: vec![BlockField::Slot],
                ..Default::default()
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&q).unwrap();
        assert!(json.contains("field_selection"));
        assert!(!json.contains("\"fields\""));
    }

    #[test]
    fn transaction_id_and_index_filters_deserialize() {
        // Dmitry Wave 2 #1/#2: filter transactions by their signature id and by index.
        let q: SolanaQuery = serde_json::from_str(
            r#"{"from_slot":0,"transactions":[{"transaction_id":["5xY..."],"transaction_index":[3,7]}]}"#,
        )
        .unwrap();
        assert_eq!(q.transactions[0].transaction_id, vec!["5xY...".to_string()]);
        assert_eq!(q.transactions[0].transaction_index, vec![3, 7]);
        assert!(!q.transactions[0].is_empty());
    }

    #[test]
    fn transaction_id_field_selectable() {
        use crate::field_selection::TransactionField;
        let q: SolanaQuery = serde_json::from_str(
            r#"{"from_slot":0,"field_selection":{"transaction":["transaction_id"]}}"#,
        )
        .unwrap();
        assert_eq!(
            q.field_selection.transaction,
            vec![TransactionField::TransactionId]
        );
    }

    #[test]
    fn legacy_include_flags_are_ignored() {
        // The per-selection `include_*` join flags have been removed. Queries that
        // still carry them must keep deserializing (the unknown keys are ignored)
        // since the server no longer handles them.
        let q: SolanaQuery = serde_json::from_str(
            r#"{"from_slot":0,"instructions":[{"program_id":["p"],"include_transaction":true,"include_logs":true}]}"#,
        )
        .unwrap();
        assert_eq!(q.instructions[0].program_id, vec!["p".to_string()]);
    }
}
