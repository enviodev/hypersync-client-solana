//! Hand-written `ProgramSchema` constants for popular pre-Anchor programs
//! whose IDL isn't available on-chain.
//!
//! Currently shipped:
//! - [`metaplex_token_metadata`] (`metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s`)
//!   first cut of six instructions.
//!
//! Documented candidates not yet bundled (PRs welcome): SPL Token, SPL
//! Token-2022, Memo, Associated Token Account, BPF Loader. Each is a small,
//! self-contained instruction set with single-byte discriminators.

pub mod mpl_token_metadata;

pub use mpl_token_metadata::metaplex_token_metadata;
