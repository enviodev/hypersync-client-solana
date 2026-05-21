//! Schema-driven Borsh interpreter.
//!
//! Walks a `&[u8]` buffer in lockstep with a `FieldType` and produces a
//! `serde_json::Value`. The schema, not Rust types or derive macros, drives
//! decoding; this is the trade-off that lets the caller decode instructions
//! whose Rust types they do not have at compile time (Anchor IDLs loaded at
//! runtime, hand-written declarative schemas, etc).
//!
//! Locked output conventions (see PLAN.md):
//! - Sub-64-bit integers render as `Value::Number`.
//! - `u64`/`i64`/`u128`/`i128` render as decimal `Value::String` (the JSON
//!   number range stops at 2^53; downstream consumers convert to `bigint`).
//! - `Pubkey` and `Array { U8, 32 }` render as base58 `Value::String`.
//! - `Bytes` (`Vec<u8>`) renders as `0x`-prefixed lowercase hex.

use super::schema::{DecodeError, DefinedTypes, FieldType, NamedField};
use serde_json::{Map, Value};

/// Slice `N` bytes off the front of `*buf` (advancing the cursor) and return
/// them as a fixed-size array. Caller's `from_le_bytes` consumes it.
fn take_arr<const N: usize>(
    buf: &mut &[u8],
    what: &'static str,
) -> Result<[u8; N], DecodeError> {
    if buf.len() < N {
        return Err(DecodeError::Underflow {
            what,
            need: N,
            have: buf.len(),
        });
    }
    let (head, tail) = buf.split_at(N);
    *buf = tail;
    Ok(head.try_into().expect("split_at guarantees length N"))
}

/// Slice a variable number of bytes off the front of `*buf`.
fn take_slice<'a>(
    buf: &mut &'a [u8],
    n: usize,
    what: &'static str,
) -> Result<&'a [u8], DecodeError> {
    if buf.len() < n {
        return Err(DecodeError::Underflow {
            what,
            need: n,
            have: buf.len(),
        });
    }
    let (head, tail) = buf.split_at(n);
    *buf = tail;
    Ok(head)
}

fn read_u32(buf: &mut &[u8], what: &'static str) -> Result<u32, DecodeError> {
    Ok(u32::from_le_bytes(take_arr::<4>(buf, what)?))
}

/// Decode a single `FieldType` from the front of `*buf`, advancing the cursor.
///
/// Returns the JSON-shaped value of that field. Trailing bytes are *not*
/// checked here; that's `decode_top_level`'s job.
pub fn decode_field(
    ty: &FieldType,
    defined: &DefinedTypes,
    buf: &mut &[u8],
) -> Result<Value, DecodeError> {
    match ty {
        FieldType::Bool => match take_arr::<1>(buf, "bool")?[0] {
            0 => Ok(Value::Bool(false)),
            1 => Ok(Value::Bool(true)),
            other => Err(DecodeError::InvalidBool(other)),
        },

        // Sub-64-bit integers go as JSON Number.
        FieldType::U8  => Ok(Value::Number(take_arr::<1>(buf, "u8")?[0].into())),
        FieldType::U16 => Ok(Value::Number(u16::from_le_bytes(take_arr(buf, "u16")?).into())),
        FieldType::U32 => Ok(Value::Number(u32::from_le_bytes(take_arr(buf, "u32")?).into())),
        FieldType::I8  => Ok(Value::Number(i8::from_le_bytes(take_arr(buf, "i8")?).into())),
        FieldType::I16 => Ok(Value::Number(i16::from_le_bytes(take_arr(buf, "i16")?).into())),
        FieldType::I32 => Ok(Value::Number(i32::from_le_bytes(take_arr(buf, "i32")?).into())),

        // ≥64-bit integers stringify (decimal).
        FieldType::U64 => Ok(Value::String(
            u64::from_le_bytes(take_arr(buf, "u64")?).to_string(),
        )),
        FieldType::U128 => Ok(Value::String(
            u128::from_le_bytes(take_arr(buf, "u128")?).to_string(),
        )),
        FieldType::I64 => Ok(Value::String(
            i64::from_le_bytes(take_arr(buf, "i64")?).to_string(),
        )),
        FieldType::I128 => Ok(Value::String(
            i128::from_le_bytes(take_arr(buf, "i128")?).to_string(),
        )),

        // Floats: serde_json::Number::from_f64 returns None for NaN/Inf;
        // fall back to Value::Null so we never panic on malformed input.
        FieldType::F32 => {
            let n = f32::from_le_bytes(take_arr(buf, "f32")?) as f64;
            Ok(serde_json::Number::from_f64(n)
                .map(Value::Number)
                .unwrap_or(Value::Null))
        }
        FieldType::F64 => {
            let n = f64::from_le_bytes(take_arr(buf, "f64")?);
            Ok(serde_json::Number::from_f64(n)
                .map(Value::Number)
                .unwrap_or(Value::Null))
        }

        FieldType::String => {
            let len = read_u32(buf, "string len")? as usize;
            let body = take_slice(buf, len, "string body")?;
            let s = std::str::from_utf8(body).map_err(|_| DecodeError::InvalidUtf8)?;
            Ok(Value::String(s.to_string()))
        }

        FieldType::Bytes => {
            let len = read_u32(buf, "bytes len")? as usize;
            let body = take_slice(buf, len, "bytes body")?;
            let mut hex = String::with_capacity(2 + body.len() * 2);
            hex.push_str("0x");
            for b in body {
                use std::fmt::Write;
                let _ = write!(&mut hex, "{:02x}", b);
            }
            Ok(Value::String(hex))
        }

        FieldType::Pubkey => {
            let bytes = take_arr::<32>(buf, "pubkey")?;
            Ok(Value::String(bs58::encode(bytes).into_string()))
        }

        FieldType::Option(inner) => match take_arr::<1>(buf, "option tag")?[0] {
            0 => Ok(Value::Null),
            1 => decode_field(inner, defined, buf),
            other => Err(DecodeError::InvalidOptionTag(other)),
        },

        FieldType::Vec(inner) => {
            let len = read_u32(buf, "vec len")? as usize;
            let mut out = Vec::with_capacity(len);
            for _ in 0..len {
                out.push(decode_field(inner, defined, buf)?);
            }
            Ok(Value::Array(out))
        }

        FieldType::Array { ty: inner, len } => {
            // Locked: [u8; 32] always renders as base58, treating any 32-byte
            // field as a pubkey by default. The non-pubkey hash/Merkle-root
            // exception is documented as a known limitation.
            if matches!(**inner, FieldType::U8) && *len == 32 {
                let bytes = take_arr::<32>(buf, "array<u8;32>")?;
                return Ok(Value::String(bs58::encode(bytes).into_string()));
            }
            let mut out = Vec::with_capacity(*len);
            for _ in 0..*len {
                out.push(decode_field(inner, defined, buf)?);
            }
            Ok(Value::Array(out))
        }

        FieldType::Struct(fields) => decode_struct_fields(fields, defined, buf),

        FieldType::Enum(variants) => {
            let idx = take_arr::<1>(buf, "enum variant index")?[0];
            let variant = variants
                .get(idx as usize)
                .ok_or(DecodeError::UnknownEnumVariant(idx))?;
            let body = match &variant.fields {
                None => Value::Object(Map::new()),
                Some(fields) => decode_struct_fields(fields, defined, buf)?,
            };
            let mut obj = Map::new();
            obj.insert(variant.name.clone(), body);
            Ok(Value::Object(obj))
        }

        FieldType::Defined(name) => {
            let resolved = defined
                .get(name)
                .ok_or_else(|| DecodeError::UnresolvedType(name.clone()))?;
            decode_field(resolved, defined, buf)
        }
    }
}

fn decode_struct_fields(
    fields: &[NamedField],
    defined: &DefinedTypes,
    buf: &mut &[u8],
) -> Result<Value, DecodeError> {
    let mut obj = Map::new();
    for f in fields {
        let v = decode_field(&f.ty, defined, buf)?;
        obj.insert(f.name.clone(), v);
    }
    Ok(Value::Object(obj))
}

/// Decode `bytes` as the given top-level `FieldType` and reject any
/// trailing data. Use this when you expect the byte stream to terminate
/// exactly at the end of the schema (e.g. instruction args after the
/// discriminator is stripped).
pub fn decode_top_level(
    ty: &FieldType,
    defined: &DefinedTypes,
    bytes: &[u8],
) -> Result<Value, DecodeError> {
    let mut buf = bytes;
    let v = decode_field(ty, defined, &mut buf)?;
    if !buf.is_empty() {
        return Err(DecodeError::TrailingBytes(buf.len()));
    }
    Ok(v)
}
