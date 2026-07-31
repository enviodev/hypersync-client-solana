//! Shared value types for the Solana HyperSync wire surface.
//!
//! The byte newtypes ([`Address`], [`Hash`], [`Signature`]) hold the decoded
//! bytes; base58 is presentation-only (`Display` / `FromStr` / serde), so the
//! JSON wire is unchanged from the plain-string era while Rust code gets type
//! safety, inline storage, and cheap comparisons.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Error parsing a base58 value into one of the fixed-size byte newtypes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBase58Error {
    /// The type being parsed, e.g. "Address".
    pub ty: &'static str,
    /// Expected byte length after decoding.
    pub expected_len: usize,
    kind: ParseBase58ErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParseBase58ErrorKind {
    Decode,
    Len(usize),
}

impl fmt::Display for ParseBase58Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ParseBase58ErrorKind::Decode => {
                write!(f, "{}: invalid base58", self.ty)
            }
            ParseBase58ErrorKind::Len(got) => write!(
                f,
                "{}: decoded to {} bytes, expected {}",
                self.ty, got, self.expected_len
            ),
        }
    }
}

impl std::error::Error for ParseBase58Error {}

macro_rules! base58_newtype {
    ($(#[$doc:meta])* $name:ident, $len:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub [u8; $len]);

        impl $name {
            pub const LEN: usize = $len;

            pub fn as_bytes(&self) -> &[u8; $len] {
                &self.0
            }
        }

        impl From<[u8; $len]> for $name {
            fn from(bytes: [u8; $len]) -> Self {
                Self(bytes)
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&bs58::encode(&self.0).into_string())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self)
            }
        }

        impl FromStr for $name {
            type Err = ParseBase58Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let mut bytes = [0u8; $len];
                let written =
                    bs58::decode(s)
                        .onto(&mut bytes[..])
                        .map_err(|e| ParseBase58Error {
                            ty: stringify!($name),
                            expected_len: $len,
                            kind: match e {
                                bs58::decode::Error::BufferTooSmall => {
                                    ParseBase58ErrorKind::Len(usize::MAX)
                                }
                                _ => ParseBase58ErrorKind::Decode,
                            },
                        })?;
                if written != $len {
                    return Err(ParseBase58Error {
                        ty: stringify!($name),
                        expected_len: $len,
                        kind: ParseBase58ErrorKind::Len(written),
                    });
                }
                Ok(Self(bytes))
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

base58_newtype!(
    /// A 32-byte Solana account address (pubkey), base58 on the wire.
    Address,
    32
);
base58_newtype!(
    /// A 32-byte hash (blockhash / recent_blockhash), base58 on the wire.
    Hash,
    32
);
base58_newtype!(
    /// A 64-byte ed25519 transaction signature (also the transaction id),
    /// base58 on the wire.
    Signature,
    64
);

/// Classification of a log line, as stored in the `logs.kind` column.
///
/// Source caveat: SQD-ingested ranges and default (canonical-fidelity) RPC
/// ranges only carry the semantic kinds `log` / `data` / `other`; the framing
/// `invoke` / `success` / `failed` / `consumed` lines are dropped for
/// cross-source row parity (see the server's solana-rpc-sqd-log-parity doc).
/// Consumers must not assume every invocation has an `invoke` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    /// "Program <id> invoke [<depth>]"
    Invoke,
    /// "Program <id> success"
    Success,
    /// "Program <id> failed: <err>"
    Failed,
    /// "Program <id> consumed <n> of <m> compute units"
    Consumed,
    /// "Program log: <msg>" (message stored with the prefix stripped)
    Log,
    /// "Program data: <base64>" (message stored with the prefix stripped)
    Data,
    /// Any line the normalizer did not recognize, kept verbatim (e.g.
    /// "Program return: <id> <data>").
    Other,
}

impl LogKind {
    /// All kinds, in declaration order.
    pub const ALL: &'static [LogKind] = &[
        LogKind::Invoke,
        LogKind::Success,
        LogKind::Failed,
        LogKind::Consumed,
        LogKind::Log,
        LogKind::Data,
        LogKind::Other,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            LogKind::Invoke => "invoke",
            LogKind::Success => "success",
            LogKind::Failed => "failed",
            LogKind::Consumed => "consumed",
            LogKind::Log => "log",
            LogKind::Data => "data",
            LogKind::Other => "other",
        }
    }

    /// Parse a stored kind string, folding anything unrecognized into
    /// [`LogKind::Other`] so a future server-side kind cannot break old
    /// clients (the raw line survives in `message`, so nothing is lost).
    pub fn from_stored(s: &str) -> Self {
        match s {
            "invoke" => LogKind::Invoke,
            "success" => LogKind::Success,
            "failed" => LogKind::Failed,
            "consumed" => LogKind::Consumed,
            "log" => LogKind::Log,
            "data" => LogKind::Data,
            _ => LogKind::Other,
        }
    }
}

impl fmt::Display for LogKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Token-side state of an `account_activity` row, derived at serving time
/// from the presence of the pre/post token columns. When selected it is
/// authoritative, so a consumer never has to infer "is this a token account"
/// from nulls (which under all-Option responses would conflate "not selected"
/// with "not a token account").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenState {
    /// The row has no token side (a native-only lamport change).
    NotAToken,
    /// The token account was created during this transaction (post only).
    Opened,
    /// The token account was closed during this transaction (pre only).
    Closed,
    /// The token account existed before and after (pre and post).
    Persisted,
}

impl TokenState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenState::NotAToken => "not_a_token",
            TokenState::Opened => "opened",
            TokenState::Closed => "closed",
            TokenState::Persisted => "persisted",
        }
    }
}

impl fmt::Display for TokenState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_round_trips_base58() {
        let s = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        let a: Address = s.parse().unwrap();
        assert_eq!(a.to_string(), s);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, format!("\"{s}\""));
        let back: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn signature_is_64_bytes() {
        // A 64-byte signature encodes to ~87-88 base58 chars.
        let sig = Signature([7u8; 64]);
        let s = sig.to_string();
        let back: Signature = s.parse().unwrap();
        assert_eq!(back, sig);
        // A 32-byte value must NOT parse as a Signature.
        let addr = Address([7u8; 32]).to_string();
        assert!(addr.parse::<Signature>().is_err());
        // And a signature-length value must not parse as an Address.
        assert!(s.parse::<Address>().is_err());
    }

    #[test]
    fn invalid_base58_errors() {
        // '0', 'I', 'O', 'l' are not in the base58 alphabet.
        assert!("0OIl".parse::<Address>().is_err());
    }

    #[test]
    fn log_kind_wire_and_stored_forms() {
        let json = serde_json::to_string(&LogKind::Consumed).unwrap();
        assert_eq!(json, "\"consumed\"");
        assert_eq!(LogKind::from_stored("consumed"), LogKind::Consumed);
        assert_eq!(LogKind::from_stored("brand_new_kind"), LogKind::Other);
        for k in LogKind::ALL {
            assert_eq!(LogKind::from_stored(k.as_str()), *k);
        }
    }

    #[test]
    fn token_state_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&TokenState::NotAToken).unwrap(),
            "\"not_a_token\""
        );
    }
}
