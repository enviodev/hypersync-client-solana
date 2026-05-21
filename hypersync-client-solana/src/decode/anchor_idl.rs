//! Anchor IDL parser.
//!
//! Supports both formats with a single code path, branching only where
//! shapes diverge:
//!
//! - **Modern (Anchor 0.30+)**: instructions ship an inline 8-byte
//!   `discriminator` array; defined-type refs are `{"defined": {"name": T}}`;
//!   account flags are `writable`/`signer`/`optional`; pubkey type is
//!   `"pubkey"`.
//! - **Legacy (Anchor 0.29)**: no inline discriminator (we compute
//!   `sha256("global:<snake_case_name>")[..8]`); defined-type refs are
//!   `{"defined": T}` (string); account flags are `isMut`/`isSigner`; pubkey
//!   type is `"publicKey"`.
//!
//! No explicit format toggle is needed: the parser accepts both shapes for
//! every divergent field. An IDL is "legacy" precisely when its instructions
//! lack an inline `discriminator` array — we compute one on the fly.

use std::collections::BTreeMap;

use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::schema::{
    EnumVariant, FieldType, InstructionSchema, NamedAccount, NamedField, ProgramSchema,
};

#[derive(Debug, Error)]
pub enum AnchorParseError {
    #[error("invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("expected JSON object at IDL root")]
    NotAnObject,

    #[error("instruction discriminator must be 8 u8s, got {0:?}")]
    BadDiscriminator(Vec<i64>),

    #[error("missing field '{0}'")]
    MissingField(&'static str),

    #[error("unsupported type shape at {path}: {value}")]
    UnsupportedType { path: String, value: String },

    #[error("type '{name}' missing 'kind' (expected 'struct', 'enum', or 'type')")]
    UnsupportedTypeKind { name: String },
}

/// Parse an Anchor IDL (modern 0.30+ or legacy 0.29) from a JSON string.
pub fn schema_from_anchor_idl_json(json: &str) -> Result<ProgramSchema, AnchorParseError> {
    let root: Value = serde_json::from_str(json)?;
    let obj = root.as_object().ok_or(AnchorParseError::NotAnObject)?;

    let program_id = obj
        .get("address")
        .and_then(Value::as_str)
        .or_else(|| {
            obj.get("metadata")
                .and_then(|m| m.get("address"))
                .and_then(Value::as_str)
        })
        .unwrap_or("")
        .to_string();

    let mut defined_types = BTreeMap::new();
    if let Some(arr) = obj.get("types").and_then(Value::as_array) {
        for t in arr {
            let name = t
                .get("name")
                .and_then(Value::as_str)
                .ok_or(AnchorParseError::MissingField("types[].name"))?
                .to_string();
            let ty_node = t
                .get("type")
                .ok_or(AnchorParseError::MissingField("types[].type"))?;
            let ty = parse_type_def(&name, ty_node)?;
            defined_types.insert(name, ty);
        }
    }

    let mut instructions: BTreeMap<Vec<u8>, InstructionSchema> = BTreeMap::new();
    let arr = obj
        .get("instructions")
        .and_then(Value::as_array)
        .ok_or(AnchorParseError::MissingField("instructions"))?;
    for ix in arr {
        let name = ix
            .get("name")
            .and_then(Value::as_str)
            .ok_or(AnchorParseError::MissingField("instructions[].name"))?
            .to_string();

        let discriminator = match ix.get("discriminator") {
            Some(Value::Array(arr)) => {
                let bytes: Vec<i64> = arr.iter().filter_map(Value::as_i64).collect();
                if bytes.len() != arr.len() || bytes.iter().any(|b| !(0..=255).contains(b)) {
                    return Err(AnchorParseError::BadDiscriminator(bytes));
                }
                bytes.into_iter().map(|b| b as u8).collect::<Vec<u8>>()
            }
            _ => legacy_discriminator(&name),
        };

        let accounts = parse_accounts(ix.get("accounts"))?;
        let args = parse_args(ix.get("args"))?;

        instructions.insert(
            discriminator.clone(),
            InstructionSchema {
                name,
                discriminator,
                accounts,
                args,
            },
        );
    }

    Ok(ProgramSchema::build(
        program_id,
        instructions,
        defined_types,
    ))
}

/// `sha256("global:" + snake_case(name))[..8]` — Anchor's pre-0.30 disc.
pub fn legacy_discriminator(name: &str) -> Vec<u8> {
    let snake = to_snake_case(name);
    let mut h = Sha256::new();
    h.update(b"global:");
    h.update(snake.as_bytes());
    h.finalize()[..8].to_vec()
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_lower = false;
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            if prev_lower {
                out.push('_');
            }
            for lc in c.to_lowercase() {
                out.push(lc);
            }
            prev_lower = false;
        } else {
            out.push(c);
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        }
    }
    out
}

fn parse_accounts(node: Option<&Value>) -> Result<Vec<NamedAccount>, AnchorParseError> {
    let Some(arr) = node.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(arr.len());
    for a in arr {
        // Composite account groups (`{ name, accounts: [...] }`) flatten into
        // the parent's account list — Anchor inlines them at the call site.
        if let Some(nested) = a.get("accounts").and_then(Value::as_array) {
            let _ = nested;
            // Recurse: nested accounts contribute in declared order.
            let nested_parsed = parse_accounts(a.get("accounts"))?;
            out.extend(nested_parsed);
            continue;
        }
        let name = a
            .get("name")
            .and_then(Value::as_str)
            .ok_or(AnchorParseError::MissingField("accounts[].name"))?
            .to_string();
        let writable = a
            .get("writable")
            .or_else(|| a.get("isMut"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let signer = a
            .get("signer")
            .or_else(|| a.get("isSigner"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let optional = a
            .get("optional")
            .or_else(|| a.get("isOptional"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        out.push(NamedAccount {
            name,
            writable,
            signer,
            optional,
        });
    }
    Ok(out)
}

fn parse_args(node: Option<&Value>) -> Result<Vec<NamedField>, AnchorParseError> {
    let Some(arr) = node.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(arr.len());
    for f in arr {
        out.push(parse_named_field(f, "args[]")?);
    }
    Ok(out)
}

fn parse_named_field(f: &Value, path: &str) -> Result<NamedField, AnchorParseError> {
    let name = f
        .get("name")
        .and_then(Value::as_str)
        .ok_or(AnchorParseError::MissingField("name"))?
        .to_string();
    let ty_node = f
        .get("type")
        .ok_or(AnchorParseError::MissingField("type"))?;
    let ty = parse_type(ty_node, path)?;
    Ok(NamedField { name, ty })
}

fn parse_type(node: &Value, path: &str) -> Result<FieldType, AnchorParseError> {
    if let Some(s) = node.as_str() {
        return Ok(parse_primitive(s));
    }
    if let Some(obj) = node.as_object() {
        if let Some(inner) = obj.get("option") {
            return Ok(FieldType::Option(Box::new(parse_type(
                inner,
                &format!("{path}.option"),
            )?)));
        }
        if let Some(inner) = obj.get("vec") {
            return Ok(FieldType::Vec(Box::new(parse_type(
                inner,
                &format!("{path}.vec"),
            )?)));
        }
        if let Some(arr) = obj.get("array").and_then(Value::as_array) {
            if arr.len() != 2 {
                return Err(AnchorParseError::UnsupportedType {
                    path: path.to_string(),
                    value: node.to_string(),
                });
            }
            let inner = parse_type(&arr[0], &format!("{path}.array[0]"))?;
            let len = arr[1].as_u64().ok_or(AnchorParseError::UnsupportedType {
                path: format!("{path}.array[1]"),
                value: arr[1].to_string(),
            })? as usize;
            return Ok(FieldType::Array {
                ty: Box::new(inner),
                len,
            });
        }
        if let Some(d) = obj.get("defined") {
            let name = if let Some(s) = d.as_str() {
                s.to_string()
            } else if let Some(name) = d.get("name").and_then(Value::as_str) {
                name.to_string()
            } else {
                return Err(AnchorParseError::UnsupportedType {
                    path: format!("{path}.defined"),
                    value: d.to_string(),
                });
            };
            return Ok(FieldType::Defined(name));
        }
    }
    Err(AnchorParseError::UnsupportedType {
        path: path.to_string(),
        value: node.to_string(),
    })
}

fn parse_primitive(s: &str) -> FieldType {
    match s {
        "bool" => FieldType::Bool,
        "u8" => FieldType::U8,
        "u16" => FieldType::U16,
        "u32" => FieldType::U32,
        "u64" => FieldType::U64,
        "u128" => FieldType::U128,
        "i8" => FieldType::I8,
        "i16" => FieldType::I16,
        "i32" => FieldType::I32,
        "i64" => FieldType::I64,
        "i128" => FieldType::I128,
        "f32" => FieldType::F32,
        "f64" => FieldType::F64,
        "string" => FieldType::String,
        "bytes" => FieldType::Bytes,
        // Modern uses "pubkey"; legacy uses "publicKey". Both → Pubkey.
        "pubkey" | "publicKey" => FieldType::Pubkey,
        // Unknown primitive falls through to Defined so users can override
        // via `defined_types`. This is a deliberate escape hatch; the IDL
        // spec doesn't extend, but tools sometimes emit nominal names here.
        other => FieldType::Defined(other.to_string()),
    }
}

fn parse_type_def(name: &str, node: &Value) -> Result<FieldType, AnchorParseError> {
    let kind = node.get("kind").and_then(Value::as_str).ok_or_else(|| {
        AnchorParseError::UnsupportedTypeKind {
            name: name.to_string(),
        }
    })?;
    match kind {
        "struct" => {
            let fields = node
                .get("fields")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .map(|f| parse_named_field(f, &format!("types.{name}.fields")))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(FieldType::Struct(fields))
        }
        "enum" => {
            let variants = node
                .get("variants")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .map(|v| parse_enum_variant(v, name))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(FieldType::Enum(variants))
        }
        // Type aliases (e.g. `pub type Foo = Pubkey`). Resolve the alias body
        // inline so the parser doesn't have to add an indirection layer.
        "type" => {
            let alias = node
                .get("alias")
                .or_else(|| node.get("type"))
                .ok_or_else(|| AnchorParseError::UnsupportedType {
                    path: format!("types.{name}.alias"),
                    value: node.to_string(),
                })?;
            parse_type(alias, &format!("types.{name}.alias"))
        }
        _ => Err(AnchorParseError::UnsupportedTypeKind {
            name: name.to_string(),
        }),
    }
}

fn parse_enum_variant(v: &Value, enum_name: &str) -> Result<EnumVariant, AnchorParseError> {
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .ok_or(AnchorParseError::MissingField("variants[].name"))?
        .to_string();
    let fields = match v.get("fields") {
        None | Some(Value::Null) => None,
        Some(Value::Array(arr)) => {
            let mut out = Vec::with_capacity(arr.len());
            for (i, f) in arr.iter().enumerate() {
                // Named struct field?
                if f.is_object() && f.get("name").and_then(Value::as_str).is_some() {
                    out.push(parse_named_field(
                        f,
                        &format!("types.{enum_name}.{name}.fields[{i}]"),
                    )?);
                } else {
                    // Tuple variant: synthesize positional names.
                    out.push(NamedField {
                        name: format!("_{i}"),
                        ty: parse_type(f, &format!("types.{enum_name}.{name}[{i}]"))?,
                    });
                }
            }
            Some(out)
        }
        Some(other) => {
            return Err(AnchorParseError::UnsupportedType {
                path: format!("types.{enum_name}.{name}.fields"),
                value: other.to_string(),
            })
        }
    };
    Ok(EnumVariant { name, fields })
}
