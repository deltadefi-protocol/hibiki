use hibiki_proto::services::{CancelAllAccountOrdersRequest, CancelAllAccountOrdersResponse};
use whisky::{calculate_tx_hash, data::PlutusDataJson, Asset, WData, WError, Wallet};

use crate::{
    config::{constant::all_hydra_to_l1_token_map, AppConfig},
    scripts::{HydraOrderBookRedeemer, Order, ScriptCache},
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{from_proto_utxos, AccountTxIndexAssetsMap},
        token::to_l1_assets,
    },
};

pub async fn handler(
    request: CancelAllAccountOrdersRequest,
    app_owner_wallet: &Wallet,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<CancelAllAccountOrdersResponse, WError> {
    let CancelAllAccountOrdersRequest {
        hydra_order_book_utxos,
        collateral_utxo,
        dex_order_book_utxo,
    } = request;

    let app_owner_vkey = &config.app_owner_vkey;

    let collateral = from_proto_utxos(&[collateral_utxo.unwrap()])[0].clone();
    let ref_input = from_proto_utxos(&[dex_order_book_utxo.unwrap()])[0].clone();
    let order_utxos = from_proto_utxos(&hydra_order_book_utxos);

    let mut account_tx_index_map = AccountTxIndexAssetsMap::new();

    let mut tx_builder = get_hydra_tx_builder();
    let hydra_account_spend = &scripts.hydra_account_spend;
    let hydra_order_book_spend = &scripts.hydra_order_book_spend;
    let hydra_order_book_withdrawal = &scripts.hydra_order_book_withdrawal;

    let order_redeemer = HydraOrderBookRedeemer::CancelOrder;

    log::info!(
        "[CANCEL_ALL_ACCOUNT_ORDERS] Cancelling {} orders",
        order_utxos.len(),
    );

    // Process each order UTxO
    for order_utxo in &order_utxos {
        let plutus_data = order_utxo
            .output
            .plutus_data
            .as_ref()
            .ok_or_else(|| WError::new("handler", "Order UTxO missing plutus_data"))?;

        let (account_id, user_account) = Order::parse_user_account(plutus_data)?;
        let user_account_json = user_account.to_json_string();

        log::debug!(
            "[CANCEL_ALL_ACCOUNT_ORDERS] Processing order UTxO: {}#{} for account: {}",
            order_utxo.input.tx_hash,
            order_utxo.input.output_index,
            account_id
        );

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

        // Filter non-lovelace assets (hydra tokens) from the order UTxO
        let non_lovelace_assets: Vec<Asset> = order_utxo
            .output
            .amount
            .iter()
            .filter(|asset| !asset.unit().is_empty() && asset.unit() != "lovelace")
            .cloned()
            .collect();

        // Create outputs for each non-lovelace asset
        for asset in non_lovelace_assets {
            log::debug!(
                "[CANCEL_ALL_ACCOUNT_ORDERS] Output tx_index: {} for account: {} asset: {} qty: {}",
                account_tx_index_map.current_index,
                account_id,
                asset.unit(),
                asset.quantity()
            );

            tx_builder
                .tx_out(&hydra_account_spend.address, &[asset.clone()])
                .tx_out_inline_datum_value(&WData::JSON(user_account_json.clone()));

            let l1_assets = to_l1_assets(std::slice::from_ref(&asset), all_hydra_to_l1_token_map())
                .map_err(WError::from_err("to_l1_assets"))?;

            account_tx_index_map.insert(&account_id, &user_account_json, &l1_assets);
        }
    }

    tx_builder
        .input_for_evaluation(&hydra_order_book_spend.ref_utxo(&collateral)?)
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
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
        .change_address(&collateral.output.address)
        .complete(None)
        .await?;

    let tx_hex = tx_builder.tx_hex();
    let tx_hash = calculate_tx_hash(&tx_hex)?;
    let signed_tx = app_owner_wallet.sign_tx(&tx_hex)?;

    let hydra_account_balance_tx_index_unit_map = account_tx_index_map.to_proto();

    log::debug!(
        "[CANCEL_ALL_ACCOUNT_ORDERS] Built tx_hex length: {}",
        tx_hex.len()
    );
    log::info!("[CANCEL_ALL_ACCOUNT_ORDERS] tx_hash: {}", tx_hash);
    log::debug!(
        "[CANCEL_ALL_ACCOUNT_ORDERS] hydra_account_balance_tx_index_unit_map: {:?}",
        hydra_account_balance_tx_index_unit_map
    );

    Ok(CancelAllAccountOrdersResponse {
        signed_tx,
        tx_hash,
        hydra_account_balance_tx_index_unit_map,
    })
}
