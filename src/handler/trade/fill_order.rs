use std::collections::HashMap;

use hibiki_proto::services::{FillOrderRequest, FillOrderResponse};
use whisky::{
    calculate_tx_hash,
    data::{ByteString, PlutusDataJson},
    PlutusDataCbor, WData, WError, Wallet,
};

use crate::{
    config::AppConfig,
    scripts::{HydraOrderBookRedeemer, Order, ScriptCache, UserAccount},
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{
            from_proto_amount, from_proto_order, from_proto_utxo, IdTxIndexMap, TxIndexAssetsMap,
        },
        token::to_hydra_token,
    },
};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub async fn handler(
    request: FillOrderRequest,
    app_owner_wallet: &Wallet,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<FillOrderResponse, WError> {
    let FillOrderRequest {
        address,
        collateral_utxo,
        orders,
        taker_order_id,
        dex_order_book_utxo,
        new_balance_outputs,
        ..
    } = request;

    let app_owner_vkey = &config.app_owner_vkey;
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let collateral = from_proto_utxo(collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(dex_order_book_utxo.as_ref().unwrap());
    let orders = orders
        .iter()
        .map(|proto_order| from_proto_order(proto_order))
        .collect::<Result<Vec<_>, _>>()?;

    let mut tx_builder = get_hydra_tx_builder();
    let hydra_account_spend = &scripts.hydra_account_spend;
    let hydra_order_book_spend = &scripts.hydra_order_book_spend;
    let hydra_order_book_withdrawal = &scripts.hydra_order_book_withdrawal;

    let mut hydra_order_utxo_tx_index_map = IdTxIndexMap::new(1); // Always at most 1 partially filled order
    let mut hydra_account_balance_tx_index_unit_map = HashMap::new();
    let mut current_index: u32 = 0;

    let order_redeemer = HydraOrderBookRedeemer::FillOrder(ByteString::new(&taker_order_id));
    for order in &orders {
        let order_utxo = &order.order_utxo;
        tx_builder
            .spending_plutus_script_v3()
            .tx_in(
                &order_utxo.input.tx_hash,
                order_utxo.input.output_index,
                &order_utxo.output.amount,
                &order_utxo.output.address,
            )
            .tx_in_inline_datum_present()
            .tx_in_redeemer_value(&hydra_order_book_spend.redeemer(order_redeemer.clone(), None))
            .spending_tx_in_reference(
                collateral.input.tx_hash.as_str(),
                hydra_order_book_spend.ref_output_index,
                &hydra_order_book_spend.hash,
                hydra_order_book_spend.size,
            )
            .input_for_evaluation(&order_utxo);
        // if fully filled -> skip
        if order.updated_order_size == 0 {
            continue;
        }

        // Create new order if partially filled
        if order.updated_order_size > 0 {
            let input_order = Order::from_cbor(&order_utxo.output.plutus_data.as_ref().unwrap())?;
            let updated_order = input_order
                .update_order(order.updated_order_size, order.updated_price_times_one_tri);
            tx_builder
                .tx_out(&hydra_order_book_spend.address, &order_utxo.output.amount)
                .tx_out_inline_datum_value(&WData::JSON(updated_order.to_json_string()));
            hydra_order_utxo_tx_index_map.add(&order.order_id);
            current_index += 1;
        }
    }

    for new_balance_output in &new_balance_outputs {
        let mut tx_index_assets_map = TxIndexAssetsMap::default();
        tx_index_assets_map.set_index(current_index);
        let account_info = new_balance_output.account.as_ref().unwrap();
        let account = UserAccount::from_proto_trade_account(&account_info, account_ops_script_hash);
        let new_balance_assets_l1 = from_proto_amount(&new_balance_output.balance_l1);
        for asset_l1 in new_balance_assets_l1 {
            tx_builder
                .tx_out(
                    &hydra_account_spend.address,
                    &to_hydra_token(&[asset_l1.clone()]),
                )
                .tx_out_inline_datum_value(&WData::JSON(account.to_json_string()));
            tx_index_assets_map.insert(&to_hydra_token(&[asset_l1]));
            current_index += 1;
        }

        if let Some(proto) = tx_index_assets_map.to_proto() {
            hydra_account_balance_tx_index_unit_map.insert(account_info.account_id.clone(), proto);
        }
    }

    tx_builder
        // reference oracle utxo
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
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

    Ok(FillOrderResponse {
        signed_tx,
        tx_hash,
        hydra_order_utxo_tx_index_map: hydra_order_utxo_tx_index_map.to_proto(),
        hydra_account_balance_tx_index_unit_map,
    })
}
