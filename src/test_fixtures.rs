//! Test fixtures module for generating valid test data with current script hashes.
//!
//! This module provides helpers to build test UTxOs and datums dynamically
//! using the current ScriptCache, so tests don't break when plutus.json is updated.

use hibiki_proto::services::{AccountInfo, Asset, Order, OrderType, UTxO, UtxoInput, UtxoOutput};
use whisky::data::{
    Address, ByteString, Constr0, Int, OutputReference, PlutusDataJson, PolicyId, ScriptHash,
    VerificationKeyHash,
};
use whisky::WData;

use crate::config::constant::dex_oracle_nft;
use crate::scripts::bar::{
    dex_account_balance_mint_minting_blueprint, dex_account_balance_spend_spending_blueprint,
    dex_order_book_spend_spending_blueprint, oracle_nft_mint_minting_blueprint,
};
use crate::scripts::{
    DexOrderBookDatum, HydraAccountIntent, HydraOrderBookIntent, HydraUserIntentDatum, ScriptCache,
    UserAccount,
};
use crate::utils::order::to_order_datum;
use crate::utils::proto::{assets_to_mvalue, from_proto_amount};
use crate::utils::token::{to_hydra_token, to_hydra_unit};

/// Configuration for test accounts
#[derive(Debug, Clone)]
pub struct TestAccountConfig {
    pub account_id: String,
    pub account_type: String,
    pub master_key: String,
    pub is_script_master_key: bool,
    pub operation_key: String,
    pub is_script_operation_key: bool,
}

impl TestAccountConfig {
    pub fn to_proto(&self) -> AccountInfo {
        AccountInfo {
            account_id: self.account_id.clone(),
            account_type: self.account_type.clone(),
            master_key: self.master_key.clone(),
            is_script_master_key: self.is_script_master_key,
            operation_key: self.operation_key.clone(),
            is_script_operation_key: self.is_script_operation_key,
        }
    }
}

#[derive(Debug, Clone)]
pub struct L2ScriptConfig {
    pub owner_vkey: String,
    pub fee_collector_vkey: String,
    pub dex_account_balance_mint_policy: String,
    pub dex_account_balance_spend_hash: String,
    pub dex_order_book_mint_policy: String,
    pub dex_order_book_spend_hash: String,
    pub dex_order_book_spend_address: String,
}

impl Default for L2ScriptConfig {
    fn default() -> Self {
        // Hardcoded OutputReference for oracle NFT minting (one-time mint UTxO)
        let oracle_nft_output_ref = OutputReference(Constr0::new(Box::new((
            ByteString::new("e293b0366c8583825f2bee9a6e77317fb49fd0e085812843de06e9546104617f"),
            Int::new(0),
        ))));

        // Derive oracle_nft_mint policy from blueprint with hardcoded OutputReference
        let oracle_nft_mint_bp = oracle_nft_mint_minting_blueprint(&oracle_nft_output_ref);
        let oracle_nft_mint_policy = PolicyId::new(&oracle_nft_mint_bp.hash);

        // dex_oracle_nft() is the DEX oracle NFT policy from env
        let dex_oracle_nft_policy = PolicyId::new(dex_oracle_nft());

        // Derive dex_account_balance_mint: param is oracle_nft_mint hash
        let dex_account_balance_mint_bp =
            dex_account_balance_mint_minting_blueprint(&oracle_nft_mint_policy);

        // Derive dex_account_balance_spend: params are (oracle_nft_mint hash, dex_oracle_nft)
        let dex_account_balance_spend_bp = dex_account_balance_spend_spending_blueprint((
            &oracle_nft_mint_policy,
            &dex_oracle_nft_policy,
        ));

        // Derive dex_order_book_spend: params are (oracle_nft_mint hash, dex_oracle_nft)
        let dex_order_book_spend_bp = dex_order_book_spend_spending_blueprint((
            &oracle_nft_mint_policy,
            &dex_oracle_nft_policy,
        ));

        Self {
            owner_vkey: "fa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c".to_string(),
            fee_collector_vkey: "c25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15"
                .to_string(),
            // dex_account_balance derived from blueprints
            dex_account_balance_mint_policy: dex_account_balance_mint_bp.hash,
            dex_account_balance_spend_hash: dex_account_balance_spend_bp.hash,
            // dex_order_book_mint_policy is the DEX oracle NFT itself
            dex_order_book_mint_policy: dex_oracle_nft().to_string(),
            // dex_order_book_spend derived from blueprint
            dex_order_book_spend_hash: dex_order_book_spend_bp.hash.clone(),
            dex_order_book_spend_address: dex_order_book_spend_bp.address.clone(),
        }
    }
}

/// DexOrderBookDatum has 12 fields:
/// 1. owner_vkey (VerificationKeyHash)
/// 2. stop_vkey (VerificationKeyHash)
/// 3. fee_account (UserAccount)
/// 4. order_book_merkle_root (ByteString - 64 hex chars)
/// 5. dex_account_balance_mint_policy (PolicyId)
/// 6. dex_account_balance_spend_address (Address)
/// 7. dex_order_book_mint_policy (PolicyId)
/// 8. dex_order_book_spend_address (Address)
/// 9. hydra_user_intent_mint_hash (ScriptHash)
/// 10. hydra_account_withdrawal_hash (ScriptHash)
/// 11. hydra_order_book_withdrawal_hash (ScriptHash)
/// 12. hydra_token_mint_policy (PolicyId)
pub fn build_dex_order_book_datum(
    scripts: &ScriptCache,
    l2_config: &L2ScriptConfig,
    fee_account: &UserAccount,
) -> DexOrderBookDatum {
    // Build whisky types for each field
    let owner_vkey = VerificationKeyHash::new(&l2_config.owner_vkey);
    let stop_vkey = VerificationKeyHash::new(&l2_config.fee_collector_vkey);
    let merkle_root =
        ByteString::new("0000000000000000000000000000000000000000000000000000000000000000");

    // L1 script policy IDs and addresses
    let dex_account_balance_mint = PolicyId::new(&l2_config.dex_account_balance_mint_policy);
    let dex_account_balance_address =
        Address::new(&l2_config.dex_account_balance_spend_hash, None, true, false);
    let dex_order_book_mint = PolicyId::new(&l2_config.dex_order_book_mint_policy);
    let dex_order_book_address =
        Address::new(&l2_config.dex_order_book_spend_hash, None, true, false);

    // Hydra L2 script hashes (from current ScriptCache)
    let hydra_user_intent_mint = ScriptHash::new(&scripts.user_intent_mint.hash);
    let hydra_account_withdrawal = ScriptHash::new(&scripts.hydra_account_withdrawal.hash);
    let hydra_order_book_withdrawal = ScriptHash::new(&scripts.hydra_order_book_withdrawal.hash);
    let hydra_token_mint = PolicyId::new(&scripts.hydra_token_mint.hash);

    // Build the DexOrderBookDatum using the Constr0 pattern
    DexOrderBookDatum(Constr0::new(Box::new((
        owner_vkey,
        stop_vkey,
        fee_account.clone(),
        merkle_root,
        dex_account_balance_mint,
        dex_account_balance_address,
        dex_order_book_mint,
        dex_order_book_address,
        hydra_user_intent_mint,
        hydra_account_withdrawal,
        hydra_order_book_withdrawal,
        hydra_token_mint,
    ))))
}

pub fn build_dex_order_book_utxo(
    scripts: &ScriptCache,
    l2_config: &L2ScriptConfig,
    tx_hash: &str,
    output_index: u32,
    fee_account: &UserAccount,
) -> Result<UTxO, String> {
    let oracle_nft = dex_oracle_nft();

    let dex_order_book_datum = build_dex_order_book_datum(scripts, l2_config, fee_account);

    // Serialize datum to CBOR hex using the PlutusDataJson trait
    let datum_wdata = WData::JSON(dex_order_book_datum.to_json_string());
    let plutus_data = datum_wdata
        .to_cbor()
        .map_err(|e| format!("Failed to encode datum: {:?}", e))?;
    let data_hash = datum_wdata
        .to_hash()
        .map_err(|e| format!("Failed to hash datum: {:?}", e))?;

    // Build the UTxO with address from L2ScriptConfig (derived from dex_oracle_nft)
    Ok(UTxO {
        input: Some(UtxoInput {
            output_index,
            tx_hash: tx_hash.to_string(),
        }),
        output: Some(UtxoOutput {
            address: l2_config.dex_order_book_spend_address.clone(),
            amount: vec![
                Asset {
                    unit: "lovelace".to_string(),
                    quantity: "6000000".to_string(),
                },
                Asset {
                    unit: oracle_nft.to_string(),
                    quantity: "1".to_string(),
                },
            ],
            data_hash: data_hash.to_string(),
            plutus_data,
            script_ref: "".to_string(),
            script_hash: "".to_string(),
        }),
    })
}

pub fn build_collateral_utxo(tx_hash: &str, output_index: u32, lovelace: &str) -> UTxO {
    UTxO {
        input: Some(UtxoInput {
            output_index,
            tx_hash: tx_hash.to_string(),
        }),
        output: Some(UtxoOutput {
            address: "addr_test1vra9zdhfa8kteyr3mfe7adkf5nlh8jl5xcg9e7pcp5w9yhq5exvwh".to_string(),
            amount: vec![Asset {
                unit: "lovelace".to_string(),
                quantity: lovelace.to_string(),
            }],
            data_hash: "".to_string(),
            plutus_data: "".to_string(),
            script_ref: "".to_string(),
            script_hash: "".to_string(),
        }),
    }
}

pub fn build_proto_order(
    order_id: &str,
    order_utxo: UTxO,
    updated_order_size: u64,
    updated_price_times_one_tri: u64,
    updated_order_value_l1: Vec<Asset>,
) -> Order {
    Order {
        order_id: order_id.to_string(),
        order_utxo: Some(order_utxo),
        updated_order_size,
        updated_price_times_one_tri,
        updated_order_value_l1,
    }
}

/// Build an order intent UTxO with proper address and datum.
/// The address comes from user_intent_spend script.
/// The datum is a HydraUserIntentDatum<HydraOrderBookIntent>::TradeIntent containing
/// the user account and PlaceOrderIntent with order datum and authorized account value.
pub fn build_order_intent_utxo(
    scripts: &ScriptCache,
    tx_hash: &str,
    output_index: u32,
    order_id: &str,
    base_token_unit: &str,
    quote_token_unit: &str,
    is_buy: bool,
    list_price_times_one_tri: u64,
    order_size: u64,
    commission_rate_bp: u64,
    user_account: &UserAccount,
    order_type: OrderType,
    authorized_account_value_l1: &[Asset],
) -> Result<UTxO, String> {
    // Build the order datum
    let order_datum = to_order_datum(
        order_id,
        &to_hydra_unit(base_token_unit),
        &to_hydra_unit(quote_token_unit),
        is_buy,
        list_price_times_one_tri,
        order_size,
        commission_rate_bp,
        user_account,
        order_type,
    );

    // Build the place order intent with authorized account value
    let place_order_intent = HydraOrderBookIntent::PlaceOrderIntent(Box::new((
        order_datum,
        assets_to_mvalue(&to_hydra_token(&from_proto_amount(authorized_account_value_l1))),
    )));

    // Build the intent datum
    let intent = HydraUserIntentDatum::TradeIntent(Box::new((
        user_account.clone(),
        place_order_intent,
    )));

    // Serialize datum to CBOR hex
    let datum_wdata = WData::JSON(intent.to_json_string());
    let plutus_data = datum_wdata
        .to_cbor()
        .map_err(|e| format!("Failed to encode intent datum: {:?}", e))?;
    let data_hash = datum_wdata
        .to_hash()
        .map_err(|e| format!("Failed to hash intent datum: {:?}", e))?;

    // Build the UTxO with user_intent_spend address and user_intent_mint NFT
    Ok(UTxO {
        input: Some(UtxoInput {
            output_index,
            tx_hash: tx_hash.to_string(),
        }),
        output: Some(UtxoOutput {
            address: scripts.user_intent_spend.address.clone(),
            amount: vec![
                Asset {
                    unit: "lovelace".to_string(),
                    quantity: "0".to_string(),
                },
                Asset {
                    unit: scripts.user_intent_mint.hash.clone(),
                    quantity: "1".to_string(),
                },
            ],
            data_hash,
            plutus_data,
            script_ref: "".to_string(),
            script_hash: "".to_string(),
        }),
    })
}

/// Build an account balance UTxO with proper address and datum.
/// The address comes from hydra_account_spend script.
/// The datum is the UserAccount.
pub fn build_account_balance_utxo(
    scripts: &ScriptCache,
    tx_hash: &str,
    output_index: u32,
    user_account: &UserAccount,
    balance_asset: &Asset,
) -> Result<UTxO, String> {
    // Serialize user account datum to CBOR hex
    let datum_wdata = WData::JSON(user_account.to_json_string());
    let plutus_data = datum_wdata
        .to_cbor()
        .map_err(|e| format!("Failed to encode account datum: {:?}", e))?;
    let data_hash = datum_wdata
        .to_hash()
        .map_err(|e| format!("Failed to hash account datum: {:?}", e))?;

    // Build the UTxO with hydra_account_spend address
    Ok(UTxO {
        input: Some(UtxoInput {
            output_index,
            tx_hash: tx_hash.to_string(),
        }),
        output: Some(UtxoOutput {
            address: scripts.hydra_account_spend.address.clone(),
            amount: vec![
                Asset {
                    unit: "lovelace".to_string(),
                    quantity: "0".to_string(),
                },
                balance_asset.clone(),
            ],
            data_hash,
            plutus_data,
            script_ref: "".to_string(),
            script_hash: "".to_string(),
        }),
    })
}

/// Build a transfer intent UTxO with proper address and datum.
/// The address comes from user_intent_spend script.
/// The datum is a HydraUserIntentDatum<HydraAccountIntent>::MasterIntent containing
/// the from_account and TransferIntent with to_account and transfer amounts.
pub fn build_transfer_intent_utxo(
    scripts: &ScriptCache,
    tx_hash: &str,
    output_index: u32,
    from_account: &UserAccount,
    to_account: &UserAccount,
    transfer_amounts_l1: &[Asset],
) -> Result<UTxO, String> {
    // Convert L1 transfer amounts to L2 hydra tokens
    let transfer_amount_l2 = assets_to_mvalue(&to_hydra_token(&from_proto_amount(transfer_amounts_l1)));

    // Build the transfer intent
    let hydra_account_intent =
        HydraAccountIntent::TransferIntent(Box::new((to_account.clone(), transfer_amount_l2)));
    let intent = HydraUserIntentDatum::MasterIntent(Box::new((
        from_account.clone(),
        hydra_account_intent,
    )));

    // Serialize datum to CBOR hex
    let datum_wdata = WData::JSON(intent.to_json_string());
    let plutus_data = datum_wdata
        .to_cbor()
        .map_err(|e| format!("Failed to encode transfer intent datum: {:?}", e))?;
    let data_hash = datum_wdata
        .to_hash()
        .map_err(|e| format!("Failed to hash transfer intent datum: {:?}", e))?;

    // Build the UTxO with user_intent_spend address and user_intent_mint NFT
    Ok(UTxO {
        input: Some(UtxoInput {
            output_index,
            tx_hash: tx_hash.to_string(),
        }),
        output: Some(UtxoOutput {
            address: scripts.user_intent_spend.address.clone(),
            amount: vec![
                Asset {
                    unit: "lovelace".to_string(),
                    quantity: "0".to_string(),
                },
                Asset {
                    unit: scripts.user_intent_mint.hash.clone(),
                    quantity: "1".to_string(),
                },
            ],
            data_hash,
            plutus_data,
            script_ref: "".to_string(),
            script_hash: "".to_string(),
        }),
    })
}

/// Build an order UTxO with proper address and datum.
/// The address comes from hydra_order_book_spend script.
/// The datum is the Order.
/// This is used for orders that have been placed and are sitting at the order book.
pub fn build_order_utxo(
    scripts: &ScriptCache,
    tx_hash: &str,
    output_index: u32,
    order_id: &str,
    base_token_unit: &str,
    quote_token_unit: &str,
    is_buy: bool,
    list_price_times_one_tri: u64,
    order_size: u64,
    commission_rate_bp: u64,
    user_account: &UserAccount,
    order_type: OrderType,
    order_value_asset: &Asset,
) -> Result<UTxO, String> {
    // Build the order datum
    let order_datum = to_order_datum(
        order_id,
        &to_hydra_unit(base_token_unit),
        &to_hydra_unit(quote_token_unit),
        is_buy,
        list_price_times_one_tri,
        order_size,
        commission_rate_bp,
        user_account,
        order_type,
    );

    // Serialize order datum to CBOR hex
    let datum_wdata = WData::JSON(order_datum.to_json_string());
    let plutus_data = datum_wdata
        .to_cbor()
        .map_err(|e| format!("Failed to encode order datum: {:?}", e))?;
    let data_hash = datum_wdata
        .to_hash()
        .map_err(|e| format!("Failed to hash order datum: {:?}", e))?;

    // Build the UTxO with hydra_order_book_spend address
    Ok(UTxO {
        input: Some(UtxoInput {
            output_index,
            tx_hash: tx_hash.to_string(),
        }),
        output: Some(UtxoOutput {
            address: scripts.hydra_order_book_spend.address.clone(),
            amount: vec![
                Asset {
                    unit: "lovelace".to_string(),
                    quantity: "0".to_string(),
                },
                order_value_asset.clone(),
            ],
            data_hash,
            plutus_data,
            script_ref: "".to_string(),
            script_hash: "".to_string(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_test_env;

    #[test]
    fn test_build_dex_order_book_datum() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                run_test_build_dex_order_book_datum();
            })
            .unwrap();

        handle.join().unwrap();
    }

    fn run_test_build_dex_order_book_datum() {
        init_test_env();

        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();

        // Create a test fee account
        let account_info = AccountInfo {
            account_id: "a810ec7a-7069-4157-aead-48ecf4d693c8".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "fdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };
        let fee_account = UserAccount::from_proto_trade_account(
            &account_info,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Build DexOrderBookDatum using whisky types directly
        let datum = build_dex_order_book_datum(&scripts, &l2_config, &fee_account);
        let json = datum.to_json_string();

        println!("DexOrderBookDatum JSON: {}", json);

        // Verify it can be serialized to CBOR
        let datum_wdata = WData::JSON(json.clone());
        let cbor_result = datum_wdata.to_cbor();
        assert!(
            cbor_result.is_ok(),
            "Failed to encode DexOrderBookDatum to CBOR: {:?}",
            cbor_result.err()
        );

        let hash_result = datum_wdata.to_hash();
        assert!(
            hash_result.is_ok(),
            "Failed to hash DexOrderBookDatum: {:?}",
            hash_result.err()
        );

        println!("DexOrderBookDatum CBOR: {}", cbor_result.unwrap());
        println!("DexOrderBookDatum Hash: {}", hash_result.unwrap());
    }

    #[test]
    fn test_build_dex_order_book_utxo() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                run_test_build_dex_order_book_utxo();
            })
            .unwrap();

        handle.join().unwrap();
    }

    fn run_test_build_dex_order_book_utxo() {
        init_test_env();

        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();

        // Create a test user account
        let account_info = AccountInfo {
            account_id: "a810ec7a-7069-4157-aead-48ecf4d693c8".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "fdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };
        let user_account = UserAccount::from_proto_trade_account(
            &account_info,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        let utxo_result = build_dex_order_book_utxo(
            &scripts,
            &l2_config,
            "4a50e9271bc74e8f143207134a5a62db89fab76eb55fbd00786487966158d86d",
            0,
            &user_account,
        );

        assert!(
            utxo_result.is_ok(),
            "Failed to build dex_order_book_utxo: {:?}",
            utxo_result.err()
        );

        let utxo = utxo_result.unwrap();
        let output = utxo.output.unwrap();
        println!("Dex Order Book UTxO plutus_data: {}", output.plutus_data);
        println!("Dex Order Book UTxO data_hash: {}", output.data_hash);
    }
}
