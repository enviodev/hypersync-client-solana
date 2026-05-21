//! Borsh-driven instruction decoder. Spike scope: schema types + interpreter
//! only. Anchor IDL parser, bundled Metaplex schema, and the public
//! `decode_instruction` entry point land in subsequent commits per
//! `PLAN.md` Phase 7b.

pub mod borsh_runtime;
pub mod schema;

pub use borsh_runtime::{decode_field, decode_top_level};
pub use schema::{DecodeError, DefinedTypes, EnumVariant, FieldType, NamedField};
