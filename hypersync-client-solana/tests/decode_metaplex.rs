//! Round-trip the bundled Metaplex Token Metadata schema against
//! `borsh::to_vec` output from a hand-derived mirror of the on-chain types.
//!
//! If `mpl-token-metadata` ever shifts a struct field, encoding via these
//! mirrors will diverge from the on-chain layout and the live test would
//! flag it. The mirrors are kept *minimal*: only the variants and types
//! reachable from the six bundled instructions.

use borsh::BorshSerialize;
use hypersync_client_solana::decode::{decode_instruction, metaplex_token_metadata};
use hypersync_client_solana::simple_types::{Address, InstructionCall};
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

/// A deterministic valid base58 account address per seed. Account arguments
/// are typed `Address` now, so fixtures must be real 32-byte pubkeys.
fn ta(seed: u8) -> String {
    Address([seed; 32]).to_string()
}

fn instr(data: Vec<u8>, accounts: Vec<String>) -> InstructionCall {
    InstructionCall {
        executing_account: "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s".parse().ok(),
        account_arguments: Some(accounts.iter().map(|a| a.parse().unwrap()).collect()),
        data: Some(data),
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

    let accounts = vec![ta(1), ta(2), ta(3), ta(4), ta(5), ta(6)];
    // 6 accounts (rent is optional in V3 and omitted)

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
    assert_eq!(decoded.named_accounts.get("metadata"), Some(&ta(1)));
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
        &instr(buf, vec![ta(1), ta(2), ta(3), ta(4), ta(5), ta(6), ta(7)]),
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

    let decoded = decode_instruction(schema, &instr(buf, vec![ta(1), ta(2)])).expect("decode");
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
    let accounts = vec![ta(1), ta(2), ta(3), ta(4), ta(5), ta(6)];
    // trailing collection_authority_record is optional, omitted
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
    let prog_id = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s".to_string();
    let accounts = vec![
        ta(1),           // authority
        prog_id.clone(), // collection_metadata absent
        ta(2),           // metadata
        prog_id.clone(), // edition absent
        ta(3),           // mint
        ta(4),           // token
        prog_id.clone(), // master_edition absent
        prog_id.clone(), // master_edition_mint absent
        prog_id.clone(), // master_edition_token absent
        prog_id.clone(), // edition_marker absent
        prog_id.clone(), // token_record absent
        ta(5),           // system_program
        ta(6),           // sysvar_instructions
        ta(7),           // spl_token_program
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
    let accounts = vec![ta(1), ta(2), ta(3), ta(4), ta(5), ta(6), ta(7), ta(8)];
    let decoded = decode_instruction(schema, &instr(buf, accounts)).expect("decode");
    assert_eq!(decoded.name, "CreateMasterEditionV3");
    assert_eq!(decoded.args, json!({ "max_supply": "1000" }));
}

#[test]
fn surplus_accounts_go_to_extra() {
    let schema = metaplex_token_metadata();
    let data = vec![0x12];
    let accounts = vec![
        ta(1),
        ta(2),
        ta(3),
        ta(4),
        ta(5),
        ta(6),
        ta(7),
        ta(8),
        ta(9),
    ];
    let decoded = decode_instruction(schema, &instr(data, accounts)).expect("decode");
    assert_eq!(decoded.extra_accounts, vec![ta(8), ta(9)]);
}
