use hibiki_proto::services::{AccountInfo, Asset as ProtoAsset, IntentTxResponse, UTxO as ProtoUTxO};
use whisky::{
    calculate_tx_hash, Asset, Budget, UTxO, UtxoInput, UtxoOutput, WData, WError, WRedeemer,
};

use crate::{
    config::AppConfig,
    constant::{dex_oracle_nft, l2_ref_scripts_index},
    scripts::{
        hydra_user_intent_mint_minting_blueprint, hydra_user_intent_spend_spending_blueprint,
        types::place_order::{
            create_place_order_mint_redeemer, create_place_order_trade_intent_datum,
        },
        OrderDetails, PlaceOrderIntent, ProtoOrderType, UserTradeAccount,
    },
    utils::{
        hydra::{get_hydra_tx_builder, get_script_ref_hex},
        proto::{assets_to_mvalue, from_proto_amount, from_proto_utxo},
        token::to_hydra_token,
    },
};

/// PlaceOrderRequest - temporary definition until proto is updated
/// TODO: Remove this once hibiki-proto is updated with PlaceOrderRequest
#[derive(Debug, Clone)]
pub struct PlaceOrderRequest {
    pub order_id: String,
    pub base_token_unit_l1: String,
    pub quote_token_unit_l1: String,
    pub is_buy: bool,
    pub list_price_times_one_tri: i64,
    pub order_size: i64,
    pub commission_rate_bp: i64,
    pub authorized_account_value_l1: Vec<ProtoAsset>,
    pub order_type: String,
    pub account: Option<AccountInfo>,
    pub address: String,
    pub collateral_utxo: Option<ProtoUTxO>,
    pub empty_utxo: Option<ProtoUTxO>,
    pub dex_order_book_utxo: Option<ProtoUTxO>,
}

/// Handler for place order requests
///
/// This creates a Hydra transaction that mints a user intent token
/// representing a place order intent on the order book.
pub async fn handler(request: PlaceOrderRequest) -> Result<IntentTxResponse, WError> {
    let AppConfig { app_owner_vkey, .. } = AppConfig::new();
    let collateral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let empty_utxo = from_proto_utxo(request.empty_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(request.dex_order_book_utxo.as_ref().unwrap());
    let account = request.account.unwrap();

    let mut tx_builder = get_hydra_tx_builder();
    let policy_id = whisky::data::PolicyId::new(dex_oracle_nft());
    let user_intent_mint = hydra_user_intent_mint_minting_blueprint(&policy_id);
    let user_intent_spend = hydra_user_intent_spend_spending_blueprint(&policy_id);

    // Create user account from proto
    let user_account = UserTradeAccount::from_proto(&account);

    // Convert L1 assets to L2 hydra tokens
    let authorized_value_l1 = from_proto_amount(&request.authorized_account_value_l1);
    let authorized_value_l2 = to_hydra_token(&authorized_value_l1);
    let authorized_value_mvalue = assets_to_mvalue(&authorized_value_l2);

    // Convert L1 token units to L2
    let base_token_l2 = to_hydra_unit(&request.base_token_unit_l1);
    let quote_token_l2 = to_hydra_unit(&request.quote_token_unit_l1);

    // Parse order type
    let order_type = ProtoOrderType::from_str(&request.order_type).to_order_type();

    // Create order details
    let order_details = OrderDetails::new(
        &request.order_id,
        &base_token_l2,
        &quote_token_l2,
        request.is_buy,
        request.list_price_times_one_tri,
        request.order_size,
        request.commission_rate_bp,
        crate::scripts::bar::UserAccount::UserTradeAccount(user_account.clone()),
        order_type,
    );

    // Create place order intent
    let place_order_intent = PlaceOrderIntent::new(order_details, authorized_value_mvalue);

    // Create datum and redeemer JSON strings
    let user_account_enum =
        crate::scripts::bar::UserAccount::UserTradeAccount(user_account.clone());
    let datum_json_str =
        create_place_order_trade_intent_datum(&user_account_enum, &place_order_intent);
    let redeemer_json_str =
        create_place_order_mint_redeemer(&user_account_enum, &place_order_intent);

    let intent_script_ref_hex = Some(get_script_ref_hex(&user_intent_mint.cbor)?);
    let intent_mint_ref_utxo = UTxO {
        input: UtxoInput {
            output_index: l2_ref_scripts_index::hydra_user_intent::MINT,
            tx_hash: collateral.input.tx_hash.clone(),
        },
        output: UtxoOutput {
            address: collateral.output.address.clone(),
            amount: Vec::new(),
            data_hash: None,
            plutus_data: None,
            script_ref: intent_script_ref_hex,
            script_hash: Some(user_intent_mint.hash.clone()),
        },
    };

    tx_builder
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        .mint_plutus_script_v3()
        .mint(1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&WRedeemer {
            data: WData::JSON(redeemer_json_str),
            ex_units: Budget::default(),
        })
        .mint_tx_in_reference(
            &collateral.input.tx_hash,
            l2_ref_scripts_index::hydra_user_intent::MINT,
            &user_intent_mint.hash,
            user_intent_mint.cbor.len() / 2,
        )
        .input_for_evaluation(&intent_mint_ref_utxo)
        .tx_out(
            &user_intent_spend.address,
            &[Asset::new_from_str(&user_intent_mint.hash, "1")],
        )
        .tx_out_inline_datum_value(&WData::JSON(datum_json_str))
        .tx_in(
            &empty_utxo.input.tx_hash,
            empty_utxo.input.output_index,
            &empty_utxo.output.amount,
            &empty_utxo.output.address,
        )
        .input_for_evaluation(&empty_utxo)
        .tx_out(&empty_utxo.output.address, &empty_utxo.output.amount)
        .required_signer_hash(&app_owner_vkey)
        .required_signer_hash(&account.operation_key)
        .tx_in_collateral(
            &collateral.input.tx_hash,
            collateral.input.output_index,
            &collateral.output.amount,
            &collateral.output.address,
        )
        .input_for_evaluation(&collateral)
        .change_address(&request.address)
        .complete(None)
        .await?;

    let tx_hex = tx_builder.tx_hex();
    let tx_hash = calculate_tx_hash(&tx_hex)?;

    Ok(IntentTxResponse {
        tx_hex,
        tx_hash,
        tx_index: 0,
        new_empty_utxo_tx_index: 1,
    })
}

/// Convert L1 token unit to L2 hydra unit
fn to_hydra_unit(unit_l1: &str) -> String {
    use crate::utils::token::blake2b_256_hex;

    let hydra_token_hash = crate::constant::hydra_token_hash();

    if unit_l1.is_empty() || unit_l1.to_lowercase() == "lovelace" {
        // Lovelace maps to just the hydra token hash (with empty asset name)
        hydra_token_hash.to_string()
    } else {
        // Other tokens: hash the L1 unit and append to hydra token hash
        let hashed_unit = blake2b_256_hex(unit_l1);
        format!("{}{}", hydra_token_hash, hashed_unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_test_env;

    #[test]
    fn test_to_hydra_unit_lovelace() {
        init_test_env();
        let result = to_hydra_unit("lovelace");
        let hydra_token_hash = crate::constant::hydra_token_hash();
        assert_eq!(result, hydra_token_hash);
    }

    #[test]
    fn test_to_hydra_unit_custom_asset() {
        init_test_env();
        let result = to_hydra_unit(
            "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
        );
        let hydra_token_hash = crate::constant::hydra_token_hash();
        assert!(result.starts_with(hydra_token_hash));
        assert!(result.len() > hydra_token_hash.len());
    }
}
