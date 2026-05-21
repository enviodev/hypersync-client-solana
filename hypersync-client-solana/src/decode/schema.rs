//! Schema types describing the Borsh layout of a Solana instruction's args
//! and accounts. Populated by either the Anchor IDL parser (future), a
//! hand-written `ProgramSchema` constant (e.g. bundled Metaplex), or directly
//! by users.

use std::collections::BTreeMap;
use thiserror::Error;

/// The Borsh-shaped type of one named field. Composite types nest via `Box`
/// (`Option`, `Vec`, `Array`, `Defined`) so a schema can describe arbitrary
/// trees without runtime allocation explosions.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    // Primitives. Borsh encodes multi-byte numerics little-endian.
    Bool,
    U8, U16, U32, U64, U128,
    I8, I16, I32, I64, I128,
    F32, F64,

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
    Array { ty: Box<FieldType>, len: usize },

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
}
