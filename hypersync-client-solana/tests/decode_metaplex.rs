//! Round-trip the bundled Metaplex Token Metadata schema against
//! `borsh::to_vec` output from a hand-derived mirror of the on-chain types.
//!
//! If `mpl-token-metadata` ever shifts a struct field, encoding via these
//! mirrors will diverge from the on-chain layout and the live test would
//! flag it. The mirrors are kept *minimal*: only the variants and types
//! reachable from the six bundled instructions.

use borsh::BorshSerialize;
use hypersync_client_solana::decode::{decode_instruction, metaplex_token_metadata};
use hypersync_client_solana::simple_types::Instruction;
use serde_json::json;

// --- on-chain layout mirrors ------------------------------------------------

#[derive(BorshSerialize)]
struct DataV2 {
    name: String,
    symbol: String,
    uri: String,
    seller_fee_basis_points: u16,
    creators: Option<Vec<Creator>>,
    collection: Option<Collection>,
    uses: Option<Uses>,
}

#[derive(BorshSerialize)]
struct Creator {
    address: [u8; 32],
    verified: bool,
    share: u8,
}

#[derive(BorshSerialize)]
struct Collection {
    verified: bool,
    key: [u8; 32],
}

#[derive(BorshSerialize)]
struct Uses {
    use_method: UseMethod,
    remaining: u64,
    total: u64,
}

#[derive(BorshSerialize)]
#[allow(dead_code)]
enum UseMethod {
    Burn,
    Multiple,
    Single,
}

#[derive(BorshSerialize)]
#[allow(dead_code)]
enum CollectionDetails {
    V1 { size: u64 },
}

#[derive(BorshSerialize)]
#[allow(dead_code)]
enum BurnArgs {
    V1 { amount: u64 },
}

// --- helpers ----------------------------------------------------------------

fn b58(b: [u8; 32]) -> String {
    bs58::encode(b).into_string()
}

fn instr(data: Vec<u8>, accounts: Vec<&str>) -> Instruction {
    Instruction {
        program_id: "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s".to_string(),
        accounts: accounts.into_iter().map(str::to_string).collect(),
        data,
        ..Default::default()
    }
}

// --- tests ------------------------------------------------------------------

#[test]
fn create_metadata_account_v3_decodes() {
    let schema = metaplex_token_metadata();
    let creator_addr: [u8; 32] = [0x11; 32];
    let collection_key: [u8; 32] = [0x22; 32];

    let data = DataV2 {
        name: "Test NFT".to_string(),
        symbol: "TST".to_string(),
        uri: "ipfs://something".to_string(),
        seller_fee_basis_points: 250,
        creators: Some(vec![Creator {
            address: creator_addr,
            verified: true,
            share: 100,
        }]),
        collection: Some(Collection {
            verified: false,
            key: collection_key,
        }),
        uses: Some(Uses {
            use_method: UseMethod::Multiple,
            remaining: 5,
            total: 10,
        }),
    };

    let mut buf: Vec<u8> = vec![0x21]; // CreateMetadataAccountV3
    buf.extend(borsh::to_vec(&data).expect("encode data"));
    buf.push(1); // is_mutable = true
                 // collection_details: Some(V1 { size: 42 })
    buf.push(1); // Option::Some
    let cd = CollectionDetails::V1 { size: 42 };
    buf.extend(borsh::to_vec(&cd).expect("encode cd"));

    let accounts = vec![
        "Metadata1",
        "Mint1",
        "MintAuth1",
        "Payer1",
        "UpdAuth1",
        "SysProg1",
    ]; // 6 accounts (rent is optional in V3 and omitted)

    let decoded = decode_instruction(schema, &instr(buf, accounts)).expect("decode");
    assert_eq!(decoded.name, "CreateMetadataAccountV3");
    assert_eq!(
        decoded.args,
        json!({
            "data": {
                "name": "Test NFT",
                "symbol": "TST",
                "uri": "ipfs://something",
                "seller_fee_basis_points": 250,
                "creators": [
                    {
                        "address": b58(creator_addr),
                        "verified": true,
                        "share": 100
                    }
                ],
                "collection": { "verified": false, "key": b58(collection_key) },
                "uses": { "use_method": { "Multiple": {} }, "remaining": "5", "total": "10" }
            },
            "is_mutable": true,
            "collection_details": { "V1": { "size": "42" } }
        })
    );
    assert_eq!(decoded.named_accounts.len(), 6);
    assert_eq!(
        decoded.named_accounts.get("metadata").map(String::as_str),
        Some("Metadata1")
    );
    assert!(decoded.extra_accounts.is_empty());
}

#[test]
fn create_metadata_account_v3_decodes_with_none_options() {
    let schema = metaplex_token_metadata();
    let data = DataV2 {
        name: "Bare".to_string(),
        symbol: "B".to_string(),
        uri: "".to_string(),
        seller_fee_basis_points: 0,
        creators: None,
        collection: None,
        uses: None,
    };
    let mut buf: Vec<u8> = vec![0x21];
    buf.extend(borsh::to_vec(&data).unwrap());
    buf.push(0); // is_mutable = false
    buf.push(0); // collection_details = None

    let decoded = decode_instruction(
        schema,
        &instr(buf, vec!["m", "mint", "ma", "p", "ua", "sys", "rent"]),
    )
    .expect("decode");
    assert_eq!(
        decoded.args,
        json!({
            "data": {
                "name": "Bare",
                "symbol": "B",
                "uri": "",
                "seller_fee_basis_points": 0,
                "creators": null,
                "collection": null,
                "uses": null
            },
            "is_mutable": false,
            "collection_details": null
        })
    );
}

#[test]
fn update_metadata_account_v2_decodes() {
    let schema = metaplex_token_metadata();
    let new_authority: [u8; 32] = [0x33; 32];
    let new_data = DataV2 {
        name: "Updated".into(),
        symbol: "UPD".into(),
        uri: "x".into(),
        seller_fee_basis_points: 0,
        creators: None,
        collection: None,
        uses: None,
    };

    let mut buf: Vec<u8> = vec![0x0f];
    buf.push(1); // data: Some
    buf.extend(borsh::to_vec(&new_data).unwrap());
    buf.push(1); // update_authority: Some
    buf.extend_from_slice(&new_authority);
    buf.push(0); // primary_sale_happened: None
    buf.push(1); // is_mutable: Some
    buf.push(0); // is_mutable = false

    let decoded = decode_instruction(schema, &instr(buf, vec!["meta", "ua"])).expect("decode");
    assert_eq!(decoded.name, "UpdateMetadataAccountV2");
    assert_eq!(
        decoded.args,
        json!({
            "data": {
                "name": "Updated", "symbol": "UPD", "uri": "x",
                "seller_fee_basis_points": 0,
                "creators": null, "collection": null, "uses": null
            },
            "update_authority": b58(new_authority),
            "primary_sale_happened": null,
            "is_mutable": false
        })
    );
}

#[test]
fn verify_collection_decodes_no_args() {
    let schema = metaplex_token_metadata();
    let data = vec![0x12];
    let accounts = vec![
        "metadata",
        "collection_authority",
        "payer",
        "collection_mint",
        "collection",
        "collection_master_edition_account",
    ]; // trailing collection_authority_record is optional, omitted
    let decoded = decode_instruction(schema, &instr(data, accounts)).expect("decode");
    assert_eq!(decoded.name, "VerifyCollection");
    assert_eq!(decoded.args, json!({}));
    assert!(decoded.extra_accounts.is_empty());
}

#[test]
fn burn_decodes_with_burn_args_enum() {
    let schema = metaplex_token_metadata();
    let mut buf = vec![0x29];
    buf.extend(borsh::to_vec(&BurnArgs::V1 { amount: 1 }).unwrap());

    // All 14 positional slots; real-world callers always supply the full
    // shape, filling absent slots with the program id.
    let prog_id = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";
    let accounts = vec![
        "authority",
        prog_id, // collection_metadata absent
        "metadata",
        prog_id, // edition absent
        "mint",
        "token",
        prog_id, // master_edition absent
        prog_id, // master_edition_mint absent
        prog_id, // master_edition_token absent
        prog_id, // edition_marker absent
        prog_id, // token_record absent
        "system_program",
        "sysvar_instructions",
        "spl_token_program",
    ];
    let decoded = decode_instruction(schema, &instr(buf, accounts)).expect("decode");
    assert_eq!(decoded.name, "Burn");
    assert_eq!(
        decoded.args,
        json!({ "burn_args": { "V1": { "amount": "1" } } })
    );
}

#[test]
fn create_master_edition_v3_decodes() {
    let schema = metaplex_token_metadata();
    let mut buf = vec![0x11];
    buf.push(1); // max_supply: Some
    buf.extend_from_slice(&1000u64.to_le_bytes());
    let accounts = vec!["ed", "mint", "ua", "ma", "payer", "meta", "tp", "sys"];
    let decoded = decode_instruction(schema, &instr(buf, accounts)).expect("decode");
    assert_eq!(decoded.name, "CreateMasterEditionV3");
    assert_eq!(decoded.args, json!({ "max_supply": "1000" }));
}

#[test]
fn surplus_accounts_go_to_extra() {
    let schema = metaplex_token_metadata();
    let data = vec![0x12];
    let accounts = vec![
        "m",
        "ca",
        "p",
        "cm",
        "c",
        "cmea",
        "car",
        "remaining_1",
        "remaining_2",
    ];
    let decoded = decode_instruction(schema, &instr(data, accounts)).expect("decode");
    assert_eq!(decoded.extra_accounts, vec!["remaining_1", "remaining_2"]);
}
