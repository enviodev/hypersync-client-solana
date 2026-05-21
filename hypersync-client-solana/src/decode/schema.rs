//! Schema types describing the Borsh layout of a Solana instruction's args
//! and accounts. Populated by either the Anchor IDL parser, a hand-written
//! `ProgramSchema` constant (e.g. bundled Metaplex), or directly by users.

use std::collections::BTreeMap;
use thiserror::Error;

/// The Borsh-shaped type of one named field. Composite types nest via `Box`
/// (`Option`, `Vec`, `Array`, `Defined`) so a schema can describe arbitrary
/// trees without runtime allocation explosions.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    // Primitives. Borsh encodes multi-byte numerics little-endian.
    Bool,
    U8,
    U16,
    U32,
    U64,
    U128,
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,

    /// Borsh `string`: `u32` LE length prefix + UTF-8 bytes.
    String,

    /// Borsh `bytes` (`Vec<u8>`): `u32` LE length prefix + raw bytes.
    /// Rendered as a `0x`-prefixed lowercase hex string.
    Bytes,

    /// 32 raw bytes interpreted as an Ed25519 public key. Rendered base58.
    Pubkey,

    /// Borsh `Option<T>`: 1-byte tag (0=None, 1=Some) + body if Some.
    Option(Box<FieldType>),

    /// Borsh `Vec<T>`: `u32` LE length prefix + N×T.
    Vec(Box<FieldType>),

    /// Borsh `[T; N]`: N×T, no length prefix. As a deliberate exception
    /// `Array { ty: U8, len: 32 }` decodes to a base58 string, matching
    /// the locked convention that all 32-byte fields surface as pubkeys.
    Array {
        ty: Box<FieldType>,
        len: usize,
    },

    /// Ordered struct: fields decoded in declared order.
    Struct(Vec<NamedField>),

    /// Borsh enum: 1-byte variant index + variant body (or nothing for unit).
    Enum(Vec<EnumVariant>),

    /// Anchor IDLs reference nominal types (`{ "defined": { "name": "T" } }`).
    /// Resolved at decode time via `ProgramSchema::defined_types`.
    Defined(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedField {
    pub name: String,
    pub ty: FieldType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    /// `None` for unit variants. `Some(vec![])` is a struct variant with no
    /// fields and decodes as `{}`; semantically equivalent but distinct.
    pub fields: Option<Vec<NamedField>>,
}

/// Lookup table for `FieldType::Defined`. Empty if the schema doesn't use
/// nominal types (typical for hand-written non-Anchor programs).
pub type DefinedTypes = BTreeMap<String, FieldType>;

/// One named account slot in an instruction (the order matches the IDL).
#[derive(Debug, Clone, PartialEq)]
pub struct NamedAccount {
    pub name: String,
    pub writable: bool,
    pub signer: bool,
    pub optional: bool,
}

/// Borsh layout + account list for one instruction within a program.
#[derive(Debug, Clone, PartialEq)]
pub struct InstructionSchema {
    pub name: String,
    /// Full discriminator bytes (8 for Anchor, 1 for most native programs).
    pub discriminator: Vec<u8>,
    pub accounts: Vec<NamedAccount>,
    pub args: Vec<NamedField>,
}

/// All instructions a program exposes, plus the IDL's `types` registry.
#[derive(Debug, Clone, PartialEq)]
pub struct ProgramSchema {
    /// Base58-encoded program ID.
    pub program_id: String,
    /// Keyed by full discriminator bytes; matched against the head of an
    /// instruction's `data` field, longest-first via `disc_lens`.
    pub instructions: BTreeMap<Vec<u8>, InstructionSchema>,
    /// Distinct discriminator lengths in *descending* order, used as the
    /// dispatch probe sequence. Built by `ProgramSchema::build`.
    pub disc_lens: Vec<usize>,
    /// Nominal types referenced via `FieldType::Defined("Name")`.
    pub defined_types: DefinedTypes,
}

impl ProgramSchema {
    /// Compute `disc_lens` from the `instructions` map's keys. Call after
    /// populating `instructions` to keep the dispatch invariant in sync.
    pub fn build(
        program_id: String,
        instructions: BTreeMap<Vec<u8>, InstructionSchema>,
        defined_types: DefinedTypes,
    ) -> Self {
        let mut lens: Vec<usize> = instructions.keys().map(|k| k.len()).collect();
        lens.sort_unstable();
        lens.dedup();
        lens.reverse();
        Self {
            program_id,
            instructions,
            disc_lens: lens,
            defined_types,
        }
    }
}

/// Result of decoding one instruction against a `ProgramSchema`.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedInstruction {
    pub name: String,
    /// Object keyed by argument name. `Value::Object({})` for no-arg
    /// instructions (e.g. `VerifyCollection`).
    pub args: serde_json::Value,
    /// IDL-faithful named accounts in declared order. Keys are exactly the
    /// names from the schema; values are base58 pubkeys.
    pub named_accounts: std::collections::BTreeMap<String, String>,
    /// Accounts beyond the named list (Anchor `remaining_accounts`, IDL
    /// drift, hand-rolled wrappers). Base58, in instruction order. `[]` when
    /// the count matches the schema exactly.
    pub extra_accounts: Vec<String>,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("buffer underflow reading {what}: need {need}, have {have}")]
    Underflow {
        what: &'static str,
        need: usize,
        have: usize,
    },

    #[error("trailing {0} byte(s) after decode")]
    TrailingBytes(usize),

    #[error("invalid bool byte: {0} (expected 0 or 1)")]
    InvalidBool(u8),

    #[error("invalid option tag: {0} (expected 0 or 1)")]
    InvalidOptionTag(u8),

    #[error("unknown enum variant index: {0}")]
    UnknownEnumVariant(u8),

    #[error("invalid UTF-8 in string")]
    InvalidUtf8,

    #[error("unresolved defined type: {0}")]
    UnresolvedType(String),

    #[error("instruction data is empty (no discriminator)")]
    EmptyInstructionData,

    #[error("unknown discriminator: 0x{}", hex_lower(.0))]
    UnknownDiscriminator(Vec<u8>),

    #[error("instruction '{instruction}' expects at least {expected} accounts, got {got}")]
    AccountCountTooFew {
        instruction: String,
        expected: usize,
        got: usize,
    },

    #[error("invalid base58 account pubkey at index {index}: {reason}")]
    InvalidAccountPubkey { index: usize, reason: String },
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}
