//! Schema-driven decoder for Solana instructions.
//!
//! Callers hand the client a [`ProgramSchema`] (loaded from an Anchor IDL, a
//! hand-written constant, or one of the [`bundled`] schemas) and a
//! [`crate::simple_types::Instruction`], and get back a
//! [`DecodedInstruction`] with named accounts and Borsh-decoded args as a
//! `serde_json::Value`.
//!
//! See `PLAN.md` Phase 7b for the locked output conventions (integer
//! stringification at the 2^53 line, base58 for all 32-byte fields,
//! `0x`-prefixed hex for `bytes`).

pub mod anchor_idl;
pub mod borsh_runtime;
pub mod bundled;
pub mod schema;

pub use anchor_idl::{schema_from_anchor_idl_json, AnchorParseError};
pub use borsh_runtime::{decode_field, decode_top_level};
pub use bundled::metaplex_token_metadata;
pub use schema::{
    DecodeError, DecodedInstruction, DefinedTypes, EnumVariant, FieldType, InstructionSchema,
    NamedAccount, NamedField, ProgramSchema,
};

use crate::simple_types::Instruction;
use std::collections::BTreeMap;

/// Decode one instruction against a `ProgramSchema`.
///
/// Discriminator dispatch tries the schema's `disc_lens` longest-first, so a
/// program mixing Anchor (8-byte) and native (1-byte) instructions works
/// without ambiguity provided no Anchor disc happens to start with a valid
/// native disc byte (highly unlikely in practice).
///
/// Account-count rule: the schema's named-account list defines the required
/// shape. The first `schema.accounts.len()` accounts pair with the schema
/// names in order; any surplus goes into `extra_accounts` (Anchor's
/// `remaining_accounts`, IDL drift, hand-rolled wrappers). If the instruction
/// supplies fewer accounts than the schema expects, the *trailing* optional
/// accounts are dropped from the requirement before erroring — matching
/// Anchor's runtime behavior for `Option<AccountInfo>` slots.
pub fn decode_instruction(
    schema: &ProgramSchema,
    instruction: &Instruction,
) -> Result<DecodedInstruction, DecodeError> {
    let data = instruction.data.as_slice();
    if data.is_empty() {
        return Err(DecodeError::EmptyInstructionData);
    }

    // Try disc lengths longest-first. `disc_lens` is pre-sorted descending.
    let mut hit: Option<(&InstructionSchema, usize)> = None;
    for &n in &schema.disc_lens {
        if data.len() < n {
            continue;
        }
        if let Some(ix) = schema.instructions.get(&data[..n]) {
            hit = Some((ix, n));
            break;
        }
    }

    let (ix, disc_len) = hit.ok_or_else(|| {
        // Surface the longest probed prefix in the error for easier debugging
        // (e.g. an 8-byte Anchor disc miss shows all 8 bytes).
        let max_n = schema
            .disc_lens
            .first()
            .copied()
            .unwrap_or(0)
            .min(data.len());
        DecodeError::UnknownDiscriminator(data[..max_n].to_vec())
    })?;

    let required = required_account_count(&ix.accounts);
    let got = instruction.accounts.len();
    if got < required {
        return Err(DecodeError::AccountCountTooFew {
            instruction: ix.name.clone(),
            expected: required,
            got,
        });
    }

    let mut named_accounts: BTreeMap<String, String> = BTreeMap::new();
    let pair_count = ix.accounts.len().min(got);
    for (i, acc) in ix.accounts[..pair_count].iter().enumerate() {
        named_accounts.insert(acc.name.clone(), instruction.accounts[i].clone());
    }

    let extra_accounts: Vec<String> = if got > ix.accounts.len() {
        instruction.accounts[ix.accounts.len()..].to_vec()
    } else {
        Vec::new()
    };

    let args_bytes = &data[disc_len..];
    let args = decode_args(&ix.args, &schema.defined_types, args_bytes)?;

    Ok(DecodedInstruction {
        name: ix.name.clone(),
        args,
        named_accounts,
        extra_accounts,
    })
}

/// How many accounts must be supplied at minimum: strip trailing optional
/// slots (Anchor's `Option<AccountInfo>` may be absent from the tail). A
/// non-optional account *after* an optional one still counts — only the
/// contiguous trailing run of optionals is droppable.
fn required_account_count(accounts: &[NamedAccount]) -> usize {
    let mut n = accounts.len();
    while n > 0 && accounts[n - 1].optional {
        n -= 1;
    }
    n
}

fn decode_args(
    args: &[NamedField],
    defined: &DefinedTypes,
    bytes: &[u8],
) -> Result<serde_json::Value, DecodeError> {
    let mut buf = bytes;
    let mut obj = serde_json::Map::new();
    for f in args {
        let v = decode_field(&f.ty, defined, &mut buf)?;
        obj.insert(f.name.clone(), v);
    }
    if !buf.is_empty() {
        return Err(DecodeError::TrailingBytes(buf.len()));
    }
    Ok(serde_json::Value::Object(obj))
}
