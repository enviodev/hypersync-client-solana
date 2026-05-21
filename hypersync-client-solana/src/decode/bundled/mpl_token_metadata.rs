//! Bundled `ProgramSchema` for Metaplex Token Metadata
//! (`metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s`).
//!
//! Six instructions covering the common metadata lifecycle:
//!
//! | Disc | Name                       | Args                                       |
//! |------|----------------------------|--------------------------------------------|
//! | 0x00 | `CreateMetadataAccount`    | `{ data: Data, is_mutable: bool }`         |
//! | 0x0f | `UpdateMetadataAccountV2`  | four `Option<_>`s                          |
//! | 0x11 | `CreateMasterEditionV3`    | `{ max_supply: Option<u64> }`              |
//! | 0x12 | `VerifyCollection`         | `()`                                       |
//! | 0x21 | `CreateMetadataAccountV3`  | `{ data: DataV2, is_mutable: bool, collection_details: Option<CollectionDetails> }` |
//! | 0x29 | `Burn` (unified V1)        | `{ burn_args: BurnArgs }`                  |
//!
//! Source of truth: `mpl-token-metadata` crate's `instruction::MetadataInstruction`
//! enum. The full instruction set (~50 variants) is left as a TODO; remaining
//! candidates include `CreateMetadataAccountV2`, `SignMetadata`, `Utilize`,
//! `Verify`, `Update` (unified), `Mint` (unified), `Transfer` (unified).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::decode::schema::{
    EnumVariant, FieldType, InstructionSchema, NamedAccount, NamedField, ProgramSchema,
};

/// Borrowed reference to a process-global cached `ProgramSchema`. Built once
/// on first call; subsequent calls return the same `&'static` reference with
/// no allocation.
pub fn metaplex_token_metadata() -> &'static ProgramSchema {
    static CACHED: OnceLock<ProgramSchema> = OnceLock::new();
    CACHED.get_or_init(build)
}

fn build() -> ProgramSchema {
    let defined = defined_types();

    let mut ix: BTreeMap<Vec<u8>, InstructionSchema> = BTreeMap::new();

    ix.insert(
        vec![0x00],
        InstructionSchema {
            name: "CreateMetadataAccount".to_string(),
            discriminator: vec![0x00],
            accounts: create_metadata_account_v1_accounts(),
            args: vec![
                NamedField {
                    name: "data".to_string(),
                    ty: FieldType::Defined("Data".to_string()),
                },
                NamedField {
                    name: "is_mutable".to_string(),
                    ty: FieldType::Bool,
                },
            ],
        },
    );

    ix.insert(
        vec![0x0f],
        InstructionSchema {
            name: "UpdateMetadataAccountV2".to_string(),
            discriminator: vec![0x0f],
            accounts: vec![
                NamedAccount {
                    name: "metadata".into(),
                    writable: true,
                    signer: false,
                    optional: false,
                },
                NamedAccount {
                    name: "update_authority".into(),
                    writable: false,
                    signer: true,
                    optional: false,
                },
            ],
            args: vec![
                NamedField {
                    name: "data".into(),
                    ty: FieldType::Option(Box::new(FieldType::Defined("DataV2".into()))),
                },
                NamedField {
                    name: "update_authority".into(),
                    ty: FieldType::Option(Box::new(FieldType::Pubkey)),
                },
                NamedField {
                    name: "primary_sale_happened".into(),
                    ty: FieldType::Option(Box::new(FieldType::Bool)),
                },
                NamedField {
                    name: "is_mutable".into(),
                    ty: FieldType::Option(Box::new(FieldType::Bool)),
                },
            ],
        },
    );

    ix.insert(
        vec![0x11],
        InstructionSchema {
            name: "CreateMasterEditionV3".to_string(),
            discriminator: vec![0x11],
            accounts: create_master_edition_v3_accounts(),
            args: vec![NamedField {
                name: "max_supply".into(),
                ty: FieldType::Option(Box::new(FieldType::U64)),
            }],
        },
    );

    ix.insert(
        vec![0x12],
        InstructionSchema {
            name: "VerifyCollection".to_string(),
            discriminator: vec![0x12],
            accounts: vec![
                NamedAccount {
                    name: "metadata".into(),
                    writable: true,
                    signer: false,
                    optional: false,
                },
                NamedAccount {
                    name: "collection_authority".into(),
                    writable: false,
                    signer: true,
                    optional: false,
                },
                NamedAccount {
                    name: "payer".into(),
                    writable: true,
                    signer: true,
                    optional: false,
                },
                NamedAccount {
                    name: "collection_mint".into(),
                    writable: false,
                    signer: false,
                    optional: false,
                },
                NamedAccount {
                    name: "collection".into(),
                    writable: false,
                    signer: false,
                    optional: false,
                },
                NamedAccount {
                    name: "collection_master_edition_account".into(),
                    writable: false,
                    signer: false,
                    optional: false,
                },
                NamedAccount {
                    name: "collection_authority_record".into(),
                    writable: false,
                    signer: false,
                    optional: true,
                },
            ],
            args: vec![],
        },
    );

    ix.insert(
        vec![0x21],
        InstructionSchema {
            name: "CreateMetadataAccountV3".to_string(),
            discriminator: vec![0x21],
            accounts: create_metadata_account_v3_accounts(),
            args: vec![
                NamedField {
                    name: "data".into(),
                    ty: FieldType::Defined("DataV2".into()),
                },
                NamedField {
                    name: "is_mutable".into(),
                    ty: FieldType::Bool,
                },
                NamedField {
                    name: "collection_details".into(),
                    ty: FieldType::Option(Box::new(FieldType::Defined("CollectionDetails".into()))),
                },
            ],
        },
    );

    ix.insert(
        vec![0x29],
        InstructionSchema {
            name: "Burn".to_string(),
            discriminator: vec![0x29],
            accounts: burn_accounts(),
            args: vec![NamedField {
                name: "burn_args".into(),
                ty: FieldType::Defined("BurnArgs".into()),
            }],
        },
    );

    ProgramSchema::build(
        "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s".to_string(),
        ix,
        defined,
    )
}

fn defined_types() -> BTreeMap<String, FieldType> {
    let mut t: BTreeMap<String, FieldType> = BTreeMap::new();

    // Data (V1) — used by CreateMetadataAccount.
    t.insert(
        "Data".into(),
        FieldType::Struct(vec![
            NamedField {
                name: "name".into(),
                ty: FieldType::String,
            },
            NamedField {
                name: "symbol".into(),
                ty: FieldType::String,
            },
            NamedField {
                name: "uri".into(),
                ty: FieldType::String,
            },
            NamedField {
                name: "seller_fee_basis_points".into(),
                ty: FieldType::U16,
            },
            NamedField {
                name: "creators".into(),
                ty: FieldType::Option(Box::new(FieldType::Vec(Box::new(FieldType::Defined(
                    "Creator".into(),
                ))))),
            },
        ]),
    );

    // DataV2 — used by Create/UpdateMetadataAccountV2/V3.
    t.insert(
        "DataV2".into(),
        FieldType::Struct(vec![
            NamedField {
                name: "name".into(),
                ty: FieldType::String,
            },
            NamedField {
                name: "symbol".into(),
                ty: FieldType::String,
            },
            NamedField {
                name: "uri".into(),
                ty: FieldType::String,
            },
            NamedField {
                name: "seller_fee_basis_points".into(),
                ty: FieldType::U16,
            },
            NamedField {
                name: "creators".into(),
                ty: FieldType::Option(Box::new(FieldType::Vec(Box::new(FieldType::Defined(
                    "Creator".into(),
                ))))),
            },
            NamedField {
                name: "collection".into(),
                ty: FieldType::Option(Box::new(FieldType::Defined("Collection".into()))),
            },
            NamedField {
                name: "uses".into(),
                ty: FieldType::Option(Box::new(FieldType::Defined("Uses".into()))),
            },
        ]),
    );

    t.insert(
        "Creator".into(),
        FieldType::Struct(vec![
            NamedField {
                name: "address".into(),
                ty: FieldType::Pubkey,
            },
            NamedField {
                name: "verified".into(),
                ty: FieldType::Bool,
            },
            NamedField {
                name: "share".into(),
                ty: FieldType::U8,
            },
        ]),
    );

    t.insert(
        "Collection".into(),
        FieldType::Struct(vec![
            NamedField {
                name: "verified".into(),
                ty: FieldType::Bool,
            },
            NamedField {
                name: "key".into(),
                ty: FieldType::Pubkey,
            },
        ]),
    );

    t.insert(
        "Uses".into(),
        FieldType::Struct(vec![
            NamedField {
                name: "use_method".into(),
                ty: FieldType::Defined("UseMethod".into()),
            },
            NamedField {
                name: "remaining".into(),
                ty: FieldType::U64,
            },
            NamedField {
                name: "total".into(),
                ty: FieldType::U64,
            },
        ]),
    );

    t.insert(
        "UseMethod".into(),
        FieldType::Enum(vec![
            EnumVariant {
                name: "Burn".into(),
                fields: None,
            },
            EnumVariant {
                name: "Multiple".into(),
                fields: None,
            },
            EnumVariant {
                name: "Single".into(),
                fields: None,
            },
        ]),
    );

    // CollectionDetails — currently only `V1 { size: u64 }` on-chain.
    t.insert(
        "CollectionDetails".into(),
        FieldType::Enum(vec![EnumVariant {
            name: "V1".into(),
            fields: Some(vec![NamedField {
                name: "size".into(),
                ty: FieldType::U64,
            }]),
        }]),
    );

    // BurnArgs::V1 { amount: u64 }.
    t.insert(
        "BurnArgs".into(),
        FieldType::Enum(vec![EnumVariant {
            name: "V1".into(),
            fields: Some(vec![NamedField {
                name: "amount".into(),
                ty: FieldType::U64,
            }]),
        }]),
    );

    t
}

fn create_metadata_account_v1_accounts() -> Vec<NamedAccount> {
    vec![
        NamedAccount {
            name: "metadata".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "mint".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "mint_authority".into(),
            writable: false,
            signer: true,
            optional: false,
        },
        NamedAccount {
            name: "payer".into(),
            writable: true,
            signer: true,
            optional: false,
        },
        NamedAccount {
            name: "update_authority".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "system_program".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "rent".into(),
            writable: false,
            signer: false,
            optional: false,
        },
    ]
}

fn create_metadata_account_v3_accounts() -> Vec<NamedAccount> {
    // CreateMetadataAccountV3 layout. `rent` became optional in this variant.
    vec![
        NamedAccount {
            name: "metadata".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "mint".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "mint_authority".into(),
            writable: false,
            signer: true,
            optional: false,
        },
        NamedAccount {
            name: "payer".into(),
            writable: true,
            signer: true,
            optional: false,
        },
        NamedAccount {
            name: "update_authority".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "system_program".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "rent".into(),
            writable: false,
            signer: false,
            optional: true,
        },
    ]
}

fn create_master_edition_v3_accounts() -> Vec<NamedAccount> {
    vec![
        NamedAccount {
            name: "edition".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "mint".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "update_authority".into(),
            writable: false,
            signer: true,
            optional: false,
        },
        NamedAccount {
            name: "mint_authority".into(),
            writable: false,
            signer: true,
            optional: false,
        },
        NamedAccount {
            name: "payer".into(),
            writable: true,
            signer: true,
            optional: false,
        },
        NamedAccount {
            name: "metadata".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "token_program".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "system_program".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "rent".into(),
            writable: false,
            signer: false,
            optional: true,
        },
    ]
}

fn burn_accounts() -> Vec<NamedAccount> {
    // Burn (unified V1) account list per mpl-token-metadata src/instructions/burn.rs.
    // Several slots are *context-dependent* (parent edition, edition marker,
    // token record), but callers supply all 14 positions even when "absent"
    // (filled with the program id). The `optional` flag is therefore purely
    // descriptive here and does not relax the decoder's positional contract.
    vec![
        NamedAccount {
            name: "authority".into(),
            writable: true,
            signer: true,
            optional: false,
        },
        NamedAccount {
            name: "collection_metadata".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "metadata".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "edition".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "mint".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "token".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "master_edition".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "master_edition_mint".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "master_edition_token".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "edition_marker".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "token_record".into(),
            writable: true,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "system_program".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "sysvar_instructions".into(),
            writable: false,
            signer: false,
            optional: false,
        },
        NamedAccount {
            name: "spl_token_program".into(),
            writable: false,
            signer: false,
            optional: false,
        },
    ]
}
