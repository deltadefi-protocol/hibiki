use hibiki_proto::services::{ProcessModifyOrderRequest, ProcessModifyOrderResponse};
use whisky::{
    calculate_tx_hash,
    data::{ByteString, PlutusData, PlutusDataJson},
    PlutusDataCbor, WData, WError, Wallet,
};

use crate::{
    config::AppConfig,
    scripts::{
        HydraAccountRedeemer, HydraOrderBookIntent, HydraOrderBookRedeemer, HydraUserIntentDatum,
        HydraUserIntentRedeemer, ScriptCache, UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{
            from_proto_amount, from_proto_balance_utxos, from_proto_order, from_proto_utxo,
            TxIndexAssetsMap,
        },
        token::to_hydra_token,
    },
};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub async fn handler(
    request: ProcessModifyOrderRequest,
    app_owner_wallet: &Wallet,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<ProcessModifyOrderResponse, WError> {
    let ProcessModifyOrderRequest {
        address,
        account,
        collateral_utxo,
        order_intent_utxo,
        order_value_l1,
        existing_order,
        account_balance_utxos,
        dex_order_book_utxo,
    } = request;

    let app_owner_vkey = &config.app_owner_vkey;
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let account = account.unwrap();
    let collateral = from_proto_utxo(collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(dex_order_book_utxo.as_ref().unwrap());
    let user_account = UserAccount::from_proto_trade_account(&account, account_ops_script_hash);
    let intent_utxo = from_proto_utxo(order_intent_utxo.as_ref().unwrap());
    let order_value = to_hydra_token(&from_proto_amount(&order_value_l1));
    let existing_order = from_proto_order(existing_order.as_ref().unwrap())?;

    let (updated_balance_l1, account_utxos) =
        from_proto_balance_utxos(account_balance_utxos.as_ref().unwrap());
    let mut unit_tx_index_map = TxIndexAssetsMap::new(updated_balance_l1.len());

    let mut tx_builder = get_hydra_tx_builder();
    let user_intent_mint = &scripts.user_intent_mint;
    let user_intent_spend = &scripts.user_intent_spend;
    let hydra_account_spend = &scripts.hydra_account_spend;
    let hydra_order_book_spend = &scripts.hydra_order_book_spend;
    let hydra_order_book_withdrawal = &scripts.hydra_order_book_withdrawal;

    let order_redeemer = HydraOrderBookRedeemer::ModifyOrder(user_account.clone());
    let intent_datum = HydraUserIntentDatum::<HydraOrderBookIntent>::from_cbor(
        &intent_utxo
            .output
            .plutus_data
            .as_deref()
            .ok_or_else(WError::from_opt(
                "process_order - intent_datum",
                "failed to parse plutus_data from intent_utxo",
            ))?,
    )?;
    let order = intent_datum.get_modified_order()?;

    tx_builder
        // Old order
        .spending_plutus_script_v3()
        .tx_in(
            &existing_order.order_utxo.input.tx_hash,
            existing_order.order_utxo.input.output_index,
            &existing_order.order_utxo.output.amount,
            &existing_order.order_utxo.output.address,
        )
        .tx_in_inline_datum_present()
        .tx_in_redeemer_value(&user_intent_spend.redeemer(ByteString::new(""), None))
        .spending_tx_in_reference(
            collateral.input.tx_hash.as_str(),
            user_intent_spend.ref_output_index,
            &user_intent_spend.hash,
            user_intent_spend.size,
        )
        .input_for_evaluation(&intent_utxo)
        .input_for_evaluation(&user_intent_spend.ref_utxo(&collateral)?)
        // New order
        .tx_out(&hydra_order_book_spend.address, &order_value)
        .tx_out_inline_datum_value(&WData::JSON(order.to_json_string()));

    for account_utxo in &account_utxos {
        tx_builder
            .spending_plutus_script_v3()
            .tx_in(
                &account_utxo.input.tx_hash,
                account_utxo.input.output_index,
                &account_utxo.output.amount,
                &account_utxo.output.address,
            )
            .tx_in_inline_datum_present()
            .tx_in_redeemer_value(&hydra_account_spend.redeemer(
                HydraAccountRedeemer::HydraAccountTrade(order_redeemer.clone()),
                None,
            ))
            .spending_tx_in_reference(
                collateral.input.tx_hash.as_str(),
                hydra_account_spend.ref_output_index,
                &hydra_account_spend.hash,
                hydra_account_spend.size,
            )
            .input_for_evaluation(&account_utxo);
    }

    unit_tx_index_map.set_index(1);
    for asset in updated_balance_l1 {
        tx_builder
            .tx_out(
                &hydra_account_spend.address,
                &to_hydra_token(std::slice::from_ref(&asset)),
            )
            .tx_out_inline_datum_value(&WData::JSON(user_account.to_json_string()));
        unit_tx_index_map.insert(&[asset]);
    }

    tx_builder
        // reference oracle utxo
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        // spending intent utxo
        .spending_plutus_script_v3()
        .tx_in(
            &intent_utxo.input.tx_hash,
            intent_utxo.input.output_index,
            &intent_utxo.output.amount,
            &intent_utxo.output.address,
        )
        .tx_in_inline_datum_present()
        .tx_in_redeemer_value(&user_intent_spend.redeemer(ByteString::new(""), None))
        .spending_tx_in_reference(
            collateral.input.tx_hash.as_str(),
            user_intent_spend.ref_output_index,
            &user_intent_spend.hash,
            user_intent_spend.size,
        )
        .input_for_evaluation(&intent_utxo)
        .input_for_evaluation(&user_intent_spend.ref_utxo(&collateral)?)
        // Burn intent
        .mint_plutus_script_v3()
        .mint(-1, &user_intent_mint.hash, "")
        .mint_redeemer_value(
            &user_intent_mint.redeemer(HydraUserIntentRedeemer::<PlutusData>::BurnIntent, None),
        )
        .mint_tx_in_reference(
            &collateral.input.tx_hash,
            user_intent_mint.ref_output_index,
            &user_intent_mint.hash,
            user_intent_mint.size,
        )
        .input_for_evaluation(&user_intent_mint.ref_utxo(&collateral)?)
        // Core withdrawal script
        .withdrawal_plutus_script_v3()
        .withdrawal(&hydra_order_book_withdrawal.address, 0)
        .withdrawal_redeemer_value(&hydra_order_book_withdrawal.redeemer(order_redeemer, None))
        .withdrawal_tx_in_reference(
            &collateral.input.tx_hash,
            hydra_order_book_withdrawal.ref_output_index,
            &hydra_order_book_withdrawal.hash,
            hydra_order_book_withdrawal.size,
        )
        .input_for_evaluation(&hydra_order_book_withdrawal.ref_utxo(&collateral)?)
        .required_signer_hash(&app_owner_vkey)
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
    let signed_tx = app_owner_wallet.sign_tx(&tx_hex)?;

    Ok(ProcessModifyOrderResponse {
        signed_tx,
        tx_hash,
        order_utxo_tx_index: 0,
        account_utxo_tx_index_unit_map: unit_tx_index_map.to_proto(),
    })
}
