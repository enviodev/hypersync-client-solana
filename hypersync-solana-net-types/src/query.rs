use serde::{Deserialize, Serialize};

use crate::types::{Address, LogKind, Signature};

/// Top-level Solana HyperSync query.
///
/// Returns block bundles matching the given filters within [from_slot, to_slot).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolanaQuery {
    /// Inclusive start slot.
    pub from_slot: u64,
    /// Exclusive end slot. If omitted, query runs to the current height.
    #[serde(default)]
    pub to_slot: Option<u64>,
    /// Instruction-call selections. A block is included if any instruction call
    /// in any of its transactions matches at least one selection.
    ///
    /// Renamed from `instructions`: one row is one runtime program invocation
    /// (an execution trace, including CPIs), the Solana counterpart to EVM traces.
    /// The legacy `instructions` key is still accepted on input via a serde alias.
    #[serde(default, alias = "instructions")]
    pub instruction_calls: Vec<InstructionSelection>,
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

    /// Per-(transaction, account) activity selections. A row is
    /// included if it matches at least one selection (empty selection `{}`
    /// matches all). Same join semantics as `balances` / `token_balances`:
    /// rows are keyed to a transaction via `transaction_index`, and requesting
    /// them does NOT force every block in the range to be returned.
    #[serde(default)]
    pub account_activity: Vec<AccountActivitySelection>,
    /// Maximum number of account activity rows to return before stopping.
    #[serde(default)]
    pub max_num_account_activity: Option<usize>,
}

/// Filter for selecting instructions.
///
/// All non-empty fields are AND-ed: an instruction must match at least one value
/// in every non-empty field. Empty fields are ignored (match-all).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstructionSelection {
    /// Match instruction calls whose executing account (the invoked program) is
    /// one of these pubkeys. Renamed from `program_id`; the legacy key is still
    /// accepted on input via a serde alias.
    #[serde(default, alias = "program_id")]
    pub executing_account: Vec<Address>,

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
    pub a0: Vec<Address>,
    /// Match account at position 1.
    #[serde(default)]
    pub a1: Vec<Address>,
    /// Match account at position 2.
    #[serde(default)]
    pub a2: Vec<Address>,
    /// Match account at position 3.
    #[serde(default)]
    pub a3: Vec<Address>,
    /// Match account at position 4.
    #[serde(default)]
    pub a4: Vec<Address>,
    /// Match account at position 5.
    #[serde(default)]
    pub a5: Vec<Address>,
    /// Match account at position 6.
    #[serde(default)]
    pub a6: Vec<Address>,
    /// Match account at position 7.
    #[serde(default)]
    pub a7: Vec<Address>,
    /// Match account at position 8.
    #[serde(default)]
    pub a8: Vec<Address>,
    /// Match account at position 9.
    #[serde(default)]
    pub a9: Vec<Address>,

    /// Filter on inner-instruction status:
    /// - None / absent: match both outer and inner
    /// - Some(true): only inner instructions
    /// - Some(false): only outer instructions
    #[serde(default)]
    pub is_inner: Option<bool>,

    /// Filter on the success of the PARENT transaction:
    /// - None / absent: match instructions of both successful and failed txs
    /// - Some(true): only instructions of successful transactions
    /// - Some(false): only instructions of failed transactions
    ///
    /// `tx_success` says nothing about the individual instruction: Solana
    /// metadata only records instructions that actually executed, so a failed
    /// transaction's rows are precisely the instructions that ran before the
    /// failure point, and every one of them has `tx_success = false`. Failed
    /// transactions land on chain and are served by default; consumers that
    /// count effects (transfers, mints, state changes) must filter
    /// `tx_success: true`, because instructions of failed transactions had
    /// their state changes rolled back.
    ///
    /// Renamed from `is_committed`; the legacy key is still accepted on input
    /// via a serde alias.
    #[serde(default, alias = "is_committed")]
    pub tx_success: Option<bool>,
}

impl InstructionSelection {
    pub fn is_empty(&self) -> bool {
        self.executing_account.is_empty()
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
            && self.tx_success.is_none()
    }
}

/// Filter for selecting transactions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionSelection {
    /// Match transactions whose fee_payer is one of these pubkeys.
    #[serde(default)]
    pub fee_payer: Vec<Address>,
    /// Match transactions by transaction id (`signatures[0]`, base58). This is the
    /// canonical Solana transaction signature and acts as the transaction's id.
    #[serde(default)]
    pub transaction_id: Vec<Signature>,
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
    pub program_id: Vec<Address>,
    /// Match logs whose kind is one of these values. Note SQD-ingested and
    /// default RPC-ingested ranges only carry `log` / `data` / `other` rows
    /// (see [`LogKind`]).
    #[serde(default)]
    pub kind: Vec<LogKind>,
}

impl LogSelection {
    pub fn is_empty(&self) -> bool {
        self.program_id.is_empty() && self.kind.is_empty()
    }
}

/// Filter for selecting rows of the unified `account_activity` table.
///
/// All non-empty fields are AND-ed: a row must match at least one value in
/// every non-empty field. Empty fields are ignored (match-all). An empty
/// selection `{}` returns every account activity row in the queried range.
///
/// `account_activity` merges the native SOL and SPL token sides into one row,
/// so a single selection can express what previously needed a `balances` and a
/// `token_balances` selection joined together.
///
/// Note that `account` means different things on the two sides: on a native
/// row it is the wallet, on a token row it is the token account. "Everything
/// for wallet W" is therefore two selections, `[{account: [W]}, {owner: [W]}]`,
/// because fields within one selection are AND-ed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountActivitySelection {
    /// Restrict to rows carrying a given side of the merge. Empty matches
    /// every row.
    ///
    /// A row that carries both sides matches either value, so
    /// `kind: ["native"]` is exactly the row set the removed `balances` table
    /// held, and `kind: ["token"]` the row set `token_balances` held.
    #[serde(default)]
    pub kind: Vec<ActivityKind>,
    /// Match by account address. For token rows this is the token account
    /// (the ATA / raw token account), matching `token_balances.account`.
    #[serde(default)]
    pub account: Vec<Address>,
    /// Match by the transaction's base58 `signatures[0]`.
    #[serde(default)]
    pub transaction_id: Vec<Signature>,
    /// Match by mint address. Only token rows carry a mint, so a non-empty
    /// mint filter restricts the result to token activity.
    #[serde(default)]
    pub mint: Vec<Address>,
    /// Match by owner (wallet) address. Matches either the pre or the post
    /// owner (the stored column is split so an in-transaction
    /// SetAuthority(AccountOwner) change stays visible).
    #[serde(default)]
    pub owner: Vec<Address>,
    /// Match by token program id (classic SPL Token vs Token-2022).
    /// Matches either the post or the pre program id.
    #[serde(default)]
    pub program_id: Vec<Address>,
    /// Match rows whose account is a transaction signer.
    ///
    /// The position flags are derived from the message header, so a source
    /// that could not derive one leaves it null. A null flag matches neither
    /// `true` nor `false`: unknown is not the same as false.
    #[serde(default)]
    pub is_signer: Option<bool>,
    /// Match rows whose account is writable. See `is_signer` on nulls.
    #[serde(default)]
    pub is_writable: Option<bool>,
    /// Match rows whose account is the transaction's fee payer. See
    /// `is_signer` on nulls.
    #[serde(default)]
    pub is_fee_payer: Option<bool>,
    /// Match rows whose account came from an address lookup table. See
    /// `is_signer` on nulls.
    #[serde(default)]
    pub from_lookup_table: Option<bool>,
}

/// Which side of a merged `account_activity` row to match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    /// The account's native SOL balance changed in this transaction.
    Native,
    /// The account appeared in this transaction's token-balance metadata.
    Token,
}

impl AccountActivitySelection {
    pub fn is_empty(&self) -> bool {
        self.kind.is_empty()
            && self.account.is_empty()
            && self.transaction_id.is_empty()
            && self.mint.is_empty()
            && self.owner.is_empty()
            && self.program_id.is_empty()
            && self.is_signer.is_none()
            && self.is_writable.is_none()
            && self.is_fee_payer.is_none()
            && self.from_lookup_table.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_selection::BlockField;

    /// A real 32-byte pubkey for filter round-trips (the SPL Token program).
    const PROG: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

    fn addr(s: &str) -> Address {
        s.parse().unwrap()
    }

    /// A valid 64-byte signature in base58.
    fn sig() -> String {
        Signature([7u8; 64]).to_string()
    }

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
        let s = sig();
        let q: SolanaQuery = serde_json::from_str(&format!(
            r#"{{"from_slot":0,"transactions":[{{"transaction_id":["{s}"],"transaction_index":[3,7]}}]}}"#,
        ))
        .unwrap();
        assert_eq!(
            q.transactions[0].transaction_id,
            vec![s.parse::<Signature>().unwrap()]
        );
        assert_eq!(q.transactions[0].transaction_index, vec![3, 7]);
        assert!(!q.transactions[0].is_empty());
    }

    /// The byte newtypes reject malformed filter values loudly instead of
    /// letting a typo'd pubkey silently match nothing.
    #[test]
    fn malformed_base58_filter_value_is_an_error() {
        let err = serde_json::from_str::<SolanaQuery>(
            r#"{"from_slot":0,"instruction_calls":[{"executing_account":["not-base58!"]}]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("base58"), "{err}");
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

    /// The query envelope - `SolanaQuery` and `SolanaFieldSelection` - denies
    /// unknown fields, so a table or field this version does not understand is
    /// an error rather than a silently different query. The failure mode this
    /// exists to prevent is a removed table selection being dropped and the
    /// query widening to match everything.
    #[test]
    fn unknown_fields_on_the_query_envelope_are_rejected() {
        // A removed top-level table selection. Without this, the query
        // deserializes with NO filters, which the server answers with the
        // entire slot range.
        let err = serde_json::from_str::<SolanaQuery>(
            r#"{"from_slot":0,"balances":[{"account":["a"]}]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("balances"), "{err}");

        // A removed field-selection table.
        let err = serde_json::from_str::<SolanaQuery>(
            r#"{"from_slot":0,"field_selection":{"balance":["slot"]}}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("balance"), "{err}");

        // A misspelled top-level key.
        let err = serde_json::from_str::<SolanaQuery>(r#"{"from_slot":0,"max_num_blockz":5}"#)
            .unwrap_err();
        assert!(err.to_string().contains("max_num_blockz"), "{err}");
    }

    /// Selections deliberately do NOT deny unknown fields.
    ///
    /// The strictness is scoped to the envelope so that callers still sending
    /// the legacy per-selection `include_*` join flags - accepted and ignored
    /// for several releases - keep working across this upgrade rather than
    /// failing outright.
    ///
    /// The known cost is that a misspelled filter field inside a selection is
    /// silently ignored, which for an AND-ed selection means it matches more
    /// rows than intended rather than fewer. That is the trade this boundary
    /// makes: the envelope catches removed tables, selections stay lenient.
    #[test]
    fn unknown_fields_inside_a_selection_are_tolerated() {
        let q: SolanaQuery = serde_json::from_str(&format!(
            r#"{{"from_slot":0,"instructions":[{{"program_id":["{PROG}"],"include_transaction":true,"include_logs":true}}]}}"#,
        ))
        .unwrap();
        assert_eq!(q.instruction_calls[0].executing_account, vec![addr(PROG)]);

        // A typo'd filter field is dropped, so the selection matches on the
        // fields it did understand.
        let q: SolanaQuery =
            serde_json::from_str(r#"{"from_slot":0,"account_activity":[{"mnt":["m"]}]}"#).unwrap();
        assert!(q.account_activity[0].mint.is_empty());
    }

    /// The renames stay wire-compatible: aliases are known field names, so
    /// deny_unknown_fields does not reject them.
    #[test]
    fn legacy_aliases_still_deserialize_under_strictness() {
        let q: SolanaQuery = serde_json::from_str(&format!(
            r#"{{"from_slot":0,"instructions":[{{"program_id":["{PROG}"]}}],"field_selection":{{"instruction":["slot"]}}}}"#,
        ))
        .unwrap();
        assert_eq!(q.instruction_calls[0].executing_account, vec![addr(PROG)]);
        assert_eq!(
            q.field_selection.instruction_call,
            vec![crate::field_selection::InstructionField::Slot]
        );
    }

    #[test]
    fn account_activity_kind_and_flag_filters_deserialize() {
        let s = sig();
        let q: SolanaQuery = serde_json::from_str(&format!(
            r#"{{"from_slot":0,"account_activity":[{{"kind":["native"],"is_fee_payer":true,"transaction_id":["{s}"]}}]}}"#,
        ))
        .unwrap();
        let sel = &q.account_activity[0];
        assert_eq!(sel.kind, vec![ActivityKind::Native]);
        assert_eq!(sel.is_fee_payer, Some(true));
        assert_eq!(sel.transaction_id, vec![s.parse::<Signature>().unwrap()]);
        assert!(!sel.is_empty());
    }

    #[test]
    fn tx_success_filter_new_and_legacy_names() {
        let q: SolanaQuery = serde_json::from_str(&format!(
            r#"{{"from_slot":0,"instruction_calls":[{{"executing_account":["{PROG}"],"tx_success":true}}]}}"#,
        ))
        .unwrap();
        assert_eq!(q.instruction_calls[0].tx_success, Some(true));

        // Legacy `is_committed` key still deserializes into tx_success.
        let q: SolanaQuery =
            serde_json::from_str(r#"{"from_slot":0,"instruction_calls":[{"is_committed":false}]}"#)
                .unwrap();
        assert_eq!(q.instruction_calls[0].tx_success, Some(false));

        // Serialization emits the new name only.
        let json = serde_json::to_string(&q).unwrap();
        assert!(json.contains("tx_success"));
        assert!(!json.contains("is_committed"));
    }

    #[test]
    fn tx_success_absent_is_none() {
        // Absent must stay tri-state None (match both) so pre-existing queries
        // keep their current behavior.
        let q: SolanaQuery =
            serde_json::from_str(r#"{"from_slot":0,"instruction_calls":[{"is_inner":false}]}"#)
                .unwrap();
        assert_eq!(q.instruction_calls[0].tx_success, None);
    }

    #[test]
    fn tx_success_alone_is_not_an_empty_selection() {
        // The server treats an empty selection as match-all and short-circuits
        // the row filter, so a selection carrying only `tx_success` must not
        // report empty or it would return every instruction instead of none.
        let q: SolanaQuery =
            serde_json::from_str(r#"{"from_slot":0,"instruction_calls":[{"tx_success":false}]}"#)
                .unwrap();
        assert!(!q.instruction_calls[0].is_empty());
        assert!(InstructionSelection::default().is_empty());
    }

    #[test]
    fn include_account_activity_is_rejected() {
        // The flag is gone: `account_activity: [{}]` expresses all-in-range,
        // and selecting the table in field_selection hydrates it. The strict
        // envelope makes the removal loud rather than silently ignored.
        let err = serde_json::from_str::<SolanaQuery>(
            r#"{"from_slot":0,"include_account_activity":true}"#,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("include_account_activity"),
            "{err}"
        );
    }

    #[test]
    fn log_kind_filter_is_typed() {
        let q: SolanaQuery =
            serde_json::from_str(r#"{"from_slot":0,"logs":[{"kind":["data","consumed"]}]}"#)
                .unwrap();
        assert_eq!(q.logs[0].kind, vec![LogKind::Data, LogKind::Consumed]);
        // An unknown kind in a FILTER is an error (unlike response decode,
        // which folds unknowns into Other).
        assert!(serde_json::from_str::<SolanaQuery>(
            r#"{"from_slot":0,"logs":[{"kind":["bogus"]}]}"#
        )
        .is_err());
    }

    #[test]
    fn instruction_calls_key_and_legacy_alias() {
        // New canonical key + new executing_account filter name.
        let q: SolanaQuery = serde_json::from_str(&format!(
            r#"{{"from_slot":0,"instruction_calls":[{{"executing_account":["{PROG}"]}}]}}"#,
        ))
        .unwrap();
        assert_eq!(q.instruction_calls.len(), 1);
        // Legacy `instructions` key still deserializes into the same field.
        let q: SolanaQuery = serde_json::from_str(&format!(
            r#"{{"from_slot":0,"instructions":[{{"program_id":["{PROG}"]}}]}}"#
        ))
        .unwrap();
        assert_eq!(q.instruction_calls.len(), 1);
        // Serialization emits the new key only.
        let json = serde_json::to_string(&q).unwrap();
        assert!(json.contains("instruction_calls"));
        assert!(!json.contains("\"instructions\""));
    }

    #[test]
    fn field_selection_instruction_call_key_and_legacy_alias() {
        use crate::field_selection::InstructionField;
        let q: SolanaQuery = serde_json::from_str(
            r#"{"from_slot":0,"field_selection":{"instruction_call":["data"]}}"#,
        )
        .unwrap();
        assert_eq!(
            q.field_selection.instruction_call,
            vec![InstructionField::Data]
        );
        // Legacy `instruction` key still works.
        let q: SolanaQuery =
            serde_json::from_str(r#"{"from_slot":0,"field_selection":{"instruction":["data"]}}"#)
                .unwrap();
        assert_eq!(
            q.field_selection.instruction_call,
            vec![InstructionField::Data]
        );
    }

    #[test]
    fn executing_account_filter_new_and_legacy() {
        // New filter name.
        let q: SolanaQuery = serde_json::from_str(&format!(
            r#"{{"from_slot":0,"instruction_calls":[{{"executing_account":["{PROG}"]}}]}}"#,
        ))
        .unwrap();
        assert_eq!(q.instruction_calls[0].executing_account, vec![addr(PROG)]);
        // Legacy `program_id` still maps to executing_account.
        let q: SolanaQuery = serde_json::from_str(&format!(
            r#"{{"from_slot":0,"instruction_calls":[{{"program_id":["{PROG}"]}}]}}"#,
        ))
        .unwrap();
        assert_eq!(q.instruction_calls[0].executing_account, vec![addr(PROG)]);
        // Serialization emits the new name.
        let json = serde_json::to_string(&q).unwrap();
        assert!(json.contains("executing_account"));
        assert!(!json.contains("program_id"));
    }

    #[test]
    fn instruction_field_renames_and_legacy_aliases() {
        use crate::field_selection::InstructionField;
        // New column names, plus the two new index columns.
        let q: SolanaQuery = serde_json::from_str(
            r#"{"from_slot":0,"field_selection":{"instruction_call":["executing_account","account_arguments","executing_account_index","account_index_arguments"]}}"#,
        )
        .unwrap();
        assert_eq!(
            q.field_selection.instruction_call,
            vec![
                InstructionField::ExecutingAccount,
                InstructionField::AccountArguments,
                InstructionField::ExecutingAccountIndex,
                InstructionField::AccountIndexArguments,
            ]
        );
        // Legacy column names still deserialize into the renamed variants.
        let q: SolanaQuery = serde_json::from_str(
            r#"{"from_slot":0,"field_selection":{"instruction":["program_id","accounts"]}}"#,
        )
        .unwrap();
        assert_eq!(
            q.field_selection.instruction_call,
            vec![
                InstructionField::ExecutingAccount,
                InstructionField::AccountArguments,
            ]
        );
    }
}
