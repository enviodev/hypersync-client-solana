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
    #[serde(default)]
    pub fields: crate::field_selection::SolanaFieldSelection,
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

    /// When true, also return the parent transaction for each matched instruction.
    #[serde(default)]
    pub include_transaction: bool,
    /// When true, also return logs associated with matched instructions.
    #[serde(default)]
    pub include_logs: bool,
    /// When true, also return inner instructions (CPIs) belonging to the same
    /// transactions as matched instructions. The client correlates inners to
    /// their parent outer via `instruction_address` prefix matching.
    #[serde(default)]
    pub include_inner_instructions: bool,
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
    /// If set, only match transactions with this success status.
    #[serde(default)]
    pub success: Option<bool>,
    /// When true, also return all instructions belonging to matched transactions.
    #[serde(default)]
    pub include_instructions: bool,
}

impl TransactionSelection {
    pub fn is_empty(&self) -> bool {
        self.fee_payer.is_empty() && self.success.is_none()
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
    /// When true, also return the parent transaction for each matched log.
    #[serde(default)]
    pub include_transaction: bool,
    /// When true, also return instructions associated with matched logs.
    #[serde(default)]
    pub include_instruction: bool,
}

impl LogSelection {
    pub fn is_empty(&self) -> bool {
        self.program_id.is_empty() && self.kind.is_empty()
    }
}
