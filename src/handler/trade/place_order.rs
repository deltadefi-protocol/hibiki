use hibiki_proto::services::{IntentTxResponse, OrderType, PlaceOrderRequest};
use whisky::{calculate_tx_hash, data::PlutusDataJson, Asset, WData, WError};

use crate::{
    config::AppConfig,
    scripts::{
        HydraOrderBookIntent, HydraUserIntentDatum, HydraUserIntentRedeemer, ScriptCache,
        UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        order::to_order_datum,
        proto::{assets_to_mvalue, from_proto_amount, from_proto_utxo},
        token::to_hydra_token,
    },
};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub async fn handler(
    request: PlaceOrderRequest,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<IntentTxResponse, WError> {
    let PlaceOrderRequest {
        address,
        account,
        collateral_utxo,
        order_id,
        is_buy,
        list_price_times_one_tri,
        order_size,
        commission_rate_bp,
        empty_utxo,
        dex_order_book_utxo,
        authorized_account_value_l1,
        base_token_unit,
        quote_token_unit,
        order_type,
    } = request;

    let app_owner_vkey = &config.app_owner_vkey;
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let account = account.unwrap();
    let collateral = from_proto_utxo(collateral_utxo.as_ref().unwrap());
    let empty_utxo = from_proto_utxo(empty_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(dex_order_book_utxo.as_ref().unwrap());
    let user_account = UserAccount::from_proto_trade_account(&account, account_ops_script_hash);

    let mut tx_builder = get_hydra_tx_builder();
    let user_intent_mint = &scripts.user_intent_mint;
    let user_intent_spend = &scripts.user_intent_spend;

    // Create transfer intent
    let order_type = OrderType::try_from(order_type).unwrap_or(OrderType::Limit);
    let order_datum = to_order_datum(
        &order_id,
        &base_token_unit,
        &quote_token_unit,
        is_buy,
        list_price_times_one_tri,
        order_size,
        commission_rate_bp,
        &user_account,
        order_type,
    );
    let place_order_intent = HydraOrderBookIntent::PlaceOrderIntent(Box::new((
        order_datum,
        assets_to_mvalue(&to_hydra_token(&from_proto_amount(
            &authorized_account_value_l1,
        ))),
    )));
    let intent = Box::new((user_account, place_order_intent));

    tx_builder
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        .mint_plutus_script_v3()
        .mint(1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&user_intent_mint.redeemer(
            HydraUserIntentRedeemer::MintMasterIntent(intent.clone()),
            None,
        ))
        .mint_tx_in_reference(
            &collateral.input.tx_hash,
            user_intent_mint.ref_output_index,
            &user_intent_mint.hash,
            user_intent_mint.size,
        )
        .input_for_evaluation(&user_intent_mint.ref_utxo(&collateral)?)
        .tx_out(
            &user_intent_spend.address,
            &[Asset::new_from_str(&user_intent_mint.hash, "1")],
        )
        .tx_out_inline_datum_value(&WData::JSON(
            HydraUserIntentDatum::MasterIntent(intent).to_json_string(),
        ))
        .tx_in(
            &empty_utxo.input.tx_hash,
            empty_utxo.input.output_index,
            &empty_utxo.output.amount,
            &empty_utxo.output.address,
        )
        .input_for_evaluation(&empty_utxo)
        .tx_out(&empty_utxo.output.address, &empty_utxo.output.amount)
        .required_signer_hash(&app_owner_vkey)
        .required_signer_hash(&account.master_key)
        .tx_in_collateral(
            &collateral.input.tx_hash,
            collateral.input.output_index,
            &collateral.output.amount,
            &collateral.output.address,
        )
        .input_for_evaluation(&collateral)
        .change_address(&address)
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
