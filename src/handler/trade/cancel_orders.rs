use hibiki_proto::services::{CancelOrdersRequest, CancelOrdersResponse};
use whisky::{calculate_tx_hash, data::PlutusDataJson, WData, WError, Wallet};

use crate::{
    config::AppConfig,
    scripts::{HydraOrderBookRedeemer, ScriptCache, UserAccount},
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{from_proto_amount, from_proto_order, from_proto_utxo, TxIndexAssetsMap},
        token::to_hydra_token,
    },
};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub async fn handler(
    request: CancelOrdersRequest,
    app_owner_wallet: &Wallet,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<CancelOrdersResponse, WError> {
    let CancelOrdersRequest {
        address,
        account,
        collateral_utxo,
        new_balance_l1,
        orders,
        dex_order_book_utxo,
    } = request;

    let app_owner_vkey = &config.app_owner_vkey;
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let account = account.unwrap();
    let collateral = from_proto_utxo(collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(dex_order_book_utxo.as_ref().unwrap());
    let user_account = UserAccount::from_proto_trade_account(&account, account_ops_script_hash);
    let orders = orders
        .iter()
        .map(|proto_order| from_proto_order(proto_order))
        .collect::<Result<Vec<_>, _>>()?;

    let updated_balance_l1 = from_proto_amount(&new_balance_l1);
    let mut unit_tx_index_map = TxIndexAssetsMap::new(updated_balance_l1.len());

    let mut tx_builder = get_hydra_tx_builder();
    let hydra_account_spend = &scripts.hydra_account_spend;
    let hydra_order_book_spend = &scripts.hydra_order_book_spend;
    let hydra_order_book_withdrawal = &scripts.hydra_order_book_withdrawal;

    let order_redeemer = HydraOrderBookRedeemer::CancelOrder;

    log::info!(
        "[CANCEL_ORDER] Cancelling {} orders for account_id: {}",
        orders.len(),
        account.account_id
    );

    for order in &orders {
        let order_utxo = &order.order_utxo;
        log::debug!(
            "[CANCEL_ORDER] Input order UTXO: {}#{} for order_id: {}",
            order_utxo.input.tx_hash,
            order_utxo.input.output_index,
            order.order_id
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
    }

    log::debug!("[CANCEL_ORDER] Account balance outputs start at tx_index: 0");
    log::debug!(
        "[CANCEL_ORDER] Processing {} updated balance assets",
        updated_balance_l1.len()
    );

    for asset in updated_balance_l1 {
        log::debug!(
            "[CANCEL_ORDER] Account balance tx_index: {} for account_id: {} asset: {} qty: {}",
            unit_tx_index_map.current_index,
            account.account_id,
            asset.unit(),
            asset.quantity()
        );
        tx_builder
            .tx_out(
                &hydra_account_spend.address,
                &to_hydra_token(&[asset.clone()]),
            )
            .tx_out_inline_datum_value(&WData::JSON(user_account.to_json_string()));
        unit_tx_index_map.insert(&[asset]);
    }

    tx_builder
        .input_for_evaluation(&hydra_order_book_spend.ref_utxo(&collateral)?)
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

    let account_utxo_tx_index_unit_map = unit_tx_index_map.to_proto();

    log::debug!("[CANCEL_ORDER] Built tx_hex length: {}", tx_hex.len());
    log::info!("[CANCEL_ORDER] tx_hash: {}", tx_hash);
    log::debug!(
        "[CANCEL_ORDER] account_utxo_tx_index_unit_map: {:?}",
        account_utxo_tx_index_unit_map
    );

    Ok(CancelOrdersResponse {
        signed_tx,
        tx_hash,
        account_utxo_tx_index_unit_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::scripts::ScriptCache;
    use crate::test_fixtures::{
        build_collateral_utxo, build_dex_order_book_utxo, build_order_utxo, build_proto_order,
        L2ScriptConfig,
    };
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;
    use hibiki_proto::services::{AccountInfo, Asset, OrderType};

    #[test]
    fn test_cancel_orders_handler() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(run_cancel_orders_case_1());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn run_cancel_orders_case_1() {
        init_test_env();

        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();

        let account = AccountInfo {
            account_id: "a810ec7a-7069-4157-aead-48ecf4d693c8".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "fdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        // Create user account for building UTxOs
        let user_account = UserAccount::from_proto_trade_account(
            &account,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Build dex_order_book_utxo dynamically with current script hashes
        let dex_order_book_utxo = build_dex_order_book_utxo(
            &scripts,
            &l2_config,
            "4a50e9271bc74e8f143207134a5a62db89fab76eb55fbd00786487966158d86d",
            0,
            &user_account,
        )
        .expect("Failed to build dex_order_book_utxo");

        // Build order_utxo dynamically with correct address and datum
        // Order value is in hydra token (quote token for buy order)
        let order_value_asset = Asset {
            unit: format!(
                "{}ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e",
                scripts.hydra_token_mint.hash
            ),
            quantity: "196569957800".to_string(),
        };
        let order_utxo = build_order_utxo(
            &scripts,
            "d7a0900ce7864f9dc375b272c17a0f886dd457731a265c1b36ab73c1828f126f",
            0,
            "7398aea0-392e-4198-8dc2-4abaf5e5afa4",
            "lovelace",
            "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
            true, // is_buy
            890900000000,
            196569957800,
            10,
            &user_account,
            OrderType::Limit,
            &order_value_asset,
        )
        .expect("Failed to build order_utxo");

        let request = CancelOrdersRequest {
            address: "addr_test1qr77kjlsarq8wy22g4flrcznjh5lkug5mvth7qhhkewgmezwvc8hnnjzy82j5twzf8dfy5gjk04yd09t488ys9605dvq4ymc4x".to_string(),
            account: Some(account),
            collateral_utxo: Some(build_collateral_utxo(
                "268896892a4e5d6fb004b235be696a990073e4984772eac156da83772a6ce176",
                0,
                "10000000",
            )),
            new_balance_l1: vec![Asset {
                unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                quantity: "196569957800".to_string(),
            }],
            orders: vec![build_proto_order(
                "7398aea0-392e-4198-8dc2-4abaf5e5afa4",
                order_utxo,
                0,
                0,
                vec![],
            )],
            dex_order_book_utxo: Some(dex_order_book_utxo),
        };

        let config = AppConfig::new();
        let app_owner_wallet = get_app_owner_wallet();

        let result = handler(request, &app_owner_wallet, &config, &scripts).await;

        match result {
            Ok(response) => {
                println!("=== Cancel Orders Result ===");
                println!("Tx Hash: {}", response.tx_hash);
                println!(
                    "Account UTxO Tx Index Unit Map: {:?}",
                    response.account_utxo_tx_index_unit_map
                );
                println!("Signed Tx: {}", response.signed_tx);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
}
