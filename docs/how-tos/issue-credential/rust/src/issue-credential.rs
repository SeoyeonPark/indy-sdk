/*
This sample is extensions of "save-schema-and-cred-def"

Shows how to issue a credential as a Trust Anchor which has created a Cred Definition
for an existing Schema.

After Trust Anchor has successfully created and stored a Cred Definition using Anonymous Credentials,
Prover's wallet is created and opened, and used to generate Prover's Master Secret.
After that, Trust Anchor generates Credential Offer for given Cred Definition, using Prover's DID
Prover uses Credential Offer to create Credential Request
Trust Anchor then uses Prover's Credential Request to issue a Credential.
Finally, Prover stores Credential in its wallet.
*/

// ------------------------------------------
// crates.io
// ------------------------------------------
#[macro_use]
extern crate serde_json;


// ------------------------------------------
// hyperledger crates
// ------------------------------------------
extern crate indyrs as indy;                      // rust wrapper project

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use indy::did;
use indy::future::Future;
use indy::ledger;
use indy::pool;
use indy::wallet;
use indy::anoncreds;

const PROTOCOL_VERSION: usize = 2;
static USEFUL_CREDENTIALS: &'static str = r#"{"key": "12345678901234567890123456789012"}"#;

fn main() {
    let wallet_name = "walletZZ";
    let pool_name = "poolZZ";

    indy::pool::set_protocol_version(PROTOCOL_VERSION).wait().unwrap();

    println!("1. Creating a new local pool ledger configuration that can be used later to connect pool nodes");
    let pool_config_file = create_genesis_txn_file_for_pool(pool_name);
    let pool_config = json!({
        "genesis_txn" : &pool_config_file
    });
    pool::create_pool_ledger_config(&pool_name, Some(&pool_config.to_string())).wait().unwrap();

    println!("2. Open pool ledger and get the pool handle from libindy");
    let pool_handle: i32 = pool::open_pool_ledger(&pool_name, None).wait().unwrap();

    println!("3. Creates a new wallet");
    let config = json!({ "id" : wallet_name.to_string() }).to_string();
    wallet::create_wallet(&config, USEFUL_CREDENTIALS).wait().unwrap();

    println!("4. Open wallet and get the wallet handle from libindy");
    let wallet_handle: i32 = wallet::open_wallet(&config, USEFUL_CREDENTIALS).wait().unwrap();

    println!("5. Generating and storing steward DID and Verkey");
    let first_json_seed = json!({
        "seed":"000000000000000000000000Steward1"
    }).to_string();
    let (steward_did, _steward_verkey) = did::create_and_store_my_did(wallet_handle, &first_json_seed).wait().unwrap();

    println!("6. Generating and storing Trust Anchor DID and Verkey");
    let (trustee_did, trustee_verkey) = did::create_and_store_my_did(wallet_handle, &"{}".to_string()).wait().unwrap();

    println!("7. Build NYM request to add Trust Anchor to the ledger");
    let build_nym_request: String = ledger::build_nym_request(&steward_did, &trustee_did, Some(&trustee_verkey), None, Some("TRUST_ANCHOR")).wait().unwrap();

    println!("8. Sending the nym request to ledger");
    let _build_nym_sign_submit_result: String = ledger::sign_and_submit_request(pool_handle, wallet_handle, &steward_did, &build_nym_request).wait().unwrap();

    println!("9. Create Schema and Build the SCHEMA request to add new schema to the ledger as a Steward");
    let name = "gvt";
    let version = "1.0";
    let attributes = r#"["age", "sex", "height", "name"]"#;
    let (_schema_id, schema_json) = anoncreds::issuer_create_schema(&steward_did, name, version, attributes).wait().unwrap();

    let build_schema_request: String = ledger::build_schema_request(&steward_did, &schema_json).wait().unwrap();

    println!("10. Sending the SCHEMA request to the ledger");
    let _signed_schema_request_response = ledger::sign_and_submit_request(pool_handle, wallet_handle, &steward_did, &build_schema_request).wait().unwrap();

    println!("11. Creating and storing CREDENTIAL DEFINITION using anoncreds as Trust Anchor, for the given Schema");
    let config_json = r#"{ "support_revocation": false }"#;
    let tag = r#"TAG1"#;
    let (cred_def_id, cred_def_json) = anoncreds::issuer_create_and_store_credential_def(wallet_handle, &trustee_did, &schema_json, tag, None, config_json).wait().unwrap();

    println!("12. Creating Prover wallet and opening it to get the handle");
    let prover_did = "VsKV7grR1BUE29mG2Fm2kX";
    let prover_wallet_name = "prover_wallet";
    let prover_wallet_config = json!({ "id" : prover_wallet_name.to_string() }).to_string();
    wallet::create_wallet(&prover_wallet_config, USEFUL_CREDENTIALS).wait().unwrap();
    let prover_wallet_handle: i32 = wallet::open_wallet(&prover_wallet_config, USEFUL_CREDENTIALS).wait().unwrap();

    println!("13. Prover is creating Master Secret");
    let master_secret_name = "master_secret";
    anoncreds::prover_create_master_secret(prover_wallet_handle, Some(master_secret_name)).wait().unwrap();

    println!("14. Issuer (Trust Anchor) is creating a Credential Offer for Prover");
    let cred_offer_json = anoncreds::issuer_create_credential_offer(wallet_handle, &cred_def_id).wait().unwrap();

    println!("15. Prover creates Credential Request");
    let (cred_req_json, cred_req_metadata_json) = anoncreds::prover_create_credential_req(prover_wallet_handle, prover_did, &cred_offer_json, &cred_def_json, &master_secret_name).wait().unwrap();

    println!("16. Issuer (Trust Anchor) creates Credential for Credential Request");

    let cred_values_json = json!({
        "sex": { "raw": "male", "encoded": "5944657099558967239210949258394887428692050081607692519917050011144233115103" },
        "name": { "raw": "Alex", "encoded": "99262857098057710338306967609588410025648622308394250666849665532448612202874" },
        "height": { "raw": "175", "encoded": "175" },
        "age": { "raw": "28", "encoded": "28" },
    });

    println!("cred_values_json = '{}'", &cred_values_json.to_string());

    let (cred_json, _cred_revoc_id, _revoc_reg_delta_json) =
        anoncreds::issuer_create_credential(wallet_handle, &cred_offer_json, &cred_req_json, &cred_values_json.to_string(), None, -1).wait().unwrap();

    println!("17. Prover processes and stores Credential");
    let out_cred_id = anoncreds::prover_store_credential(prover_wallet_handle, None, &cred_req_metadata_json, &cred_json, &cred_def_json, None).wait().unwrap();

    println!("Stored Credential ID is {}", &out_cred_id);

    // Clean UP
    println!("17. Close and delete two wallets");
    wallet::close_wallet(prover_wallet_handle).wait().unwrap();
    wallet::delete_wallet(&prover_wallet_config, USEFUL_CREDENTIALS).wait().unwrap();
    wallet::close_wallet(wallet_handle).wait().unwrap();
    wallet::delete_wallet(&config, USEFUL_CREDENTIALS).wait().unwrap();

    println!("18. Close pool and delete pool ledger config");
    pool::close_pool_ledger(pool_handle).wait().unwrap();
    pool::delete_pool_ledger(&pool_name).wait().unwrap();
}

fn create_genesis_txn_file_for_pool(pool_name: &str) -> String {
    let test_pool_ip = env::var("TEST_POOL_IP").unwrap_or("127.0.0.1".to_string());

    let node_txns = format!(
        r#"{{"reqSignature":{{}},"txn":{{"data":{{"data":{{"alias":"Node1","blskey":"4N8aUNHSgjQVgkpm8nhNEfDf6txHznoYREg9kirmJrkivgL4oSEimFF6nsQ6M41QvhM2Z33nves5vfSn9n1UwNFJBYtWVnHYMATn76vLuL3zU88KyeAYcHfsih3He6UHcXDxcaecHVz6jhCYz1P2UZn2bDVruL5wXpehgBfBaLKm3Ba","blskey_pop":"RahHYiCvoNCtPTrVtP7nMC5eTYrsUA8WjXbdhNc8debh1agE9bGiJxWBXYNFbnJXoXhWFMvyqhqhRoq737YQemH5ik9oL7R4NTTCz2LEZhkgLJzB3QRQqJyBNyv7acbdHrAT8nQ9UkLbaVL9NBpnWXBTw4LEMePaSHEw66RzPNdAX1","client_ip":"{0}","client_port":9702,"node_ip":"{0}","node_port":9701,"services":["VALIDATOR"]}},"dest":"Gw6pDLhcBcoQesN72qfotTgFa7cbuqZpkX3Xo6pLhPhv"}},"metadata":{{"from":"Th7MpTaRZVRYnPiabds81Y"}},"type":"0"}},"txnMetadata":{{"seqNo":1,"txnId":"fea82e10e894419fe2bea7d96296a6d46f50f93f9eeda954ec461b2ed2950b62"}},"ver":"1"}}
           {{"reqSignature":{{}},"txn":{{"data":{{"data":{{"alias":"Node2","blskey":"37rAPpXVoxzKhz7d9gkUe52XuXryuLXoM6P6LbWDB7LSbG62Lsb33sfG7zqS8TK1MXwuCHj1FKNzVpsnafmqLG1vXN88rt38mNFs9TENzm4QHdBzsvCuoBnPH7rpYYDo9DZNJePaDvRvqJKByCabubJz3XXKbEeshzpz4Ma5QYpJqjk","blskey_pop":"Qr658mWZ2YC8JXGXwMDQTzuZCWF7NK9EwxphGmcBvCh6ybUuLxbG65nsX4JvD4SPNtkJ2w9ug1yLTj6fgmuDg41TgECXjLCij3RMsV8CwewBVgVN67wsA45DFWvqvLtu4rjNnE9JbdFTc1Z4WCPA3Xan44K1HoHAq9EVeaRYs8zoF5","client_ip":"{0}","client_port":9704,"node_ip":"{0}","node_port":9703,"services":["VALIDATOR"]}},"dest":"8ECVSk179mjsjKRLWiQtssMLgp6EPhWXtaYyStWPSGAb"}},"metadata":{{"from":"EbP4aYNeTHL6q385GuVpRV"}},"type":"0"}},"txnMetadata":{{"seqNo":2,"txnId":"1ac8aece2a18ced660fef8694b61aac3af08ba875ce3026a160acbc3a3af35fc"}},"ver":"1"}}
           {{"reqSignature":{{}},"txn":{{"data":{{"data":{{"alias":"Node3","blskey":"3WFpdbg7C5cnLYZwFZevJqhubkFALBfCBBok15GdrKMUhUjGsk3jV6QKj6MZgEubF7oqCafxNdkm7eswgA4sdKTRc82tLGzZBd6vNqU8dupzup6uYUf32KTHTPQbuUM8Yk4QFXjEf2Usu2TJcNkdgpyeUSX42u5LqdDDpNSWUK5deC5","blskey_pop":"QwDeb2CkNSx6r8QC8vGQK3GRv7Yndn84TGNijX8YXHPiagXajyfTjoR87rXUu4G4QLk2cF8NNyqWiYMus1623dELWwx57rLCFqGh7N4ZRbGDRP4fnVcaKg1BcUxQ866Ven4gw8y4N56S5HzxXNBZtLYmhGHvDtk6PFkFwCvxYrNYjh","client_ip":"{0}","client_port":9706,"node_ip":"{0}","node_port":9705,"services":["VALIDATOR"]}},"dest":"DKVxG2fXXTU8yT5N7hGEbXB3dfdAnYv1JczDUHpmDxya"}},"metadata":{{"from":"4cU41vWW82ArfxJxHkzXPG"}},"type":"0"}},"txnMetadata":{{"seqNo":3,"txnId":"7e9f355dffa78ed24668f0e0e369fd8c224076571c51e2ea8be5f26479edebe4"}},"ver":"1"}}
           {{"reqSignature":{{}},"txn":{{"data":{{"data":{{"alias":"Node4","blskey":"2zN3bHM1m4rLz54MJHYSwvqzPchYp8jkHswveCLAEJVcX6Mm1wHQD1SkPYMzUDTZvWvhuE6VNAkK3KxVeEmsanSmvjVkReDeBEMxeDaayjcZjFGPydyey1qxBHmTvAnBKoPydvuTAqx5f7YNNRAdeLmUi99gERUU7TD8KfAa6MpQ9bw","blskey_pop":"RPLagxaR5xdimFzwmzYnz4ZhWtYQEj8iR5ZU53T2gitPCyCHQneUn2Huc4oeLd2B2HzkGnjAff4hWTJT6C7qHYB1Mv2wU5iHHGFWkhnTX9WsEAbunJCV2qcaXScKj4tTfvdDKfLiVuU2av6hbsMztirRze7LvYBkRHV3tGwyCptsrP","client_ip":"{0}","client_port":9708,"node_ip":"{0}","node_port":9707,"services":["VALIDATOR"]}},"dest":"4PS3EDQ3dW1tci1Bp6543CfuuebjFrg36kLAUcskGfaA"}},"metadata":{{"from":"TWwCRQRZ2ZHMJFn9TzLp7W"}},"type":"0"}},"txnMetadata":{{"seqNo":4,"txnId":"aa5e817d7cc626170eca175822029339a444eb0ee8f0bd20d3b0b76e566fb008"}},"ver":"1"}}"#, test_pool_ip);

    let pool_config_pathbuf = write_genesis_txn_to_file(pool_name, node_txns.as_str());
    pool_config_pathbuf.as_os_str().to_str().unwrap().to_string()
}

fn write_genesis_txn_to_file(pool_name: &str,
                             txn_file_data: &str) -> PathBuf {
    let mut txn_file_path = env::temp_dir();
    txn_file_path.push("indy_client");
    txn_file_path.push(format!("{}.txn", pool_name));

    if !txn_file_path.parent().unwrap().exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .create(txn_file_path.parent().unwrap()).unwrap();
    }

    let mut f = fs::File::create(txn_file_path.as_path()).unwrap();
    f.write_all(txn_file_data.as_bytes()).unwrap();
    f.flush().unwrap();
    f.sync_all().unwrap();

    txn_file_path
}