use std::time::Instant;

use hibiki_proto::services::{ProcessOrderRequest, ProcessOrderResponse};
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
        proto::{from_proto_amount, from_proto_balance_utxos, from_proto_utxo, TxIndexAssetsMap},
        token::to_hydra_token,
    },
};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub async fn handler(
    request: ProcessOrderRequest,
    app_owner_wallet: &Wallet,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<ProcessOrderResponse, WError> {
    let ProcessOrderRequest {
        address,
        account,
        collateral_utxo,
        order_intent_utxo,
        order_value_l1,
        account_balance_utxos,
        dex_order_book_utxo,
    } = request;

    let start = Instant::now();
    let app_owner_vkey = &config.app_owner_vkey;
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let account = account.unwrap();
    let collateral = from_proto_utxo(collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(dex_order_book_utxo.as_ref().unwrap());
    let user_account = UserAccount::from_proto_trade_account(&account, account_ops_script_hash);
    let intent_utxo = from_proto_utxo(order_intent_utxo.as_ref().unwrap());
    let order_value = to_hydra_token(&from_proto_amount(&order_value_l1));

    let (updated_balance_l1, account_utxos) =
        from_proto_balance_utxos(account_balance_utxos.as_ref().unwrap());
    let mut unit_tx_index_map = TxIndexAssetsMap::new(updated_balance_l1.len());

    let mut tx_builder = get_hydra_tx_builder();
    let user_intent_mint = &scripts.user_intent_mint;
    let user_intent_spend = &scripts.user_intent_spend;
    let hydra_account_spend = &scripts.hydra_account_spend;
    let hydra_order_book_spend = &scripts.hydra_order_book_spend;
    let hydra_order_book_withdrawal = &scripts.hydra_order_book_withdrawal;

    let order_redeemer = HydraOrderBookRedeemer::PlaceOrder(user_account.clone());
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

    let order = intent_datum.get_placed_order()?;
    log::info!("[PROCESS_ORDER] Processing order for account_id: {}", account.account_id);
    log::debug!("[PROCESS_ORDER] Order output at tx_index: 0");

    tx_builder
        .tx_out(&hydra_order_book_spend.address, &order_value)
        .tx_out_inline_datum_value(&WData::JSON(order.to_json_string()));

    log::debug!("[PROCESS_ORDER] Consuming {} account balance UTXOs", account_utxos.len());
    for account_utxo in &account_utxos {
        log::debug!(
            "[CONSUME_UTXO] Process order consuming account balance UTXO: {}#{} for account_id: {}",
            account_utxo.input.tx_hash, account_utxo.input.output_index, account.account_id
        );
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

    log::debug!("[PROCESS_ORDER] Account balance outputs start at tx_index: 1");
    log::debug!("[PROCESS_ORDER] Processing {} updated balance assets", updated_balance_l1.len());
    unit_tx_index_map.set_index(1);
    for asset in updated_balance_l1 {
        log::debug!(
            "[PROCESS_ORDER] Account balance tx_index: {} for account_id: {} asset: {} qty: {}",
            unit_tx_index_map.current_index, account.account_id, asset.unit(), asset.quantity()
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
        .input_for_evaluation(&hydra_account_spend.ref_utxo(&collateral)?)
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

    let account_utxo_tx_index_unit_map = unit_tx_index_map.to_proto();

    log::debug!("[PROCESS_ORDER] Built tx_hex length: {}", tx_hex.len());
    log::info!("[PROCESS_ORDER] tx_hash: {} completed in {:?}", tx_hash, start.elapsed());
    log::debug!(
        "[PROCESS_ORDER] account_utxo_tx_index_unit_map: {:?}",
        account_utxo_tx_index_unit_map
    );

    Ok(ProcessOrderResponse {
        signed_tx,
        tx_hash,
        order_utxo_tx_index: 0,
        account_utxo_tx_index_unit_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::scripts::ScriptCache;
    use crate::test_fixtures::{
        build_account_balance_utxo, build_collateral_utxo, build_dex_order_book_utxo,
        build_order_intent_utxo, L2ScriptConfig,
    };
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;
    use hibiki_proto::services::{AccountInfo, Asset, BalanceUtxos, OrderType};

    #[test]
    fn test_process_order_handler() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(test_process_order_case());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn test_process_order_case() {
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

        // Build order_intent_utxo dynamically with correct address and datum
        let authorized_account_value_l1 = vec![Asset {
            unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
            quantity: "199999922800".to_string(),
        }];
        let order_intent_utxo = build_order_intent_utxo(
            &scripts,
            "dfa5aa1c5a699ee5bfc12925d0cc6fa86ec091e156d44316d59099df4e7f5ac4",
            0,
            "7398aea0-392e-4198-8dc2-4abaf5e5afa4",
            "lovelace",
            "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
            true, // is_buy
            890900000000,
            199999922800,
            10,
            &user_account,
            OrderType::Limit,
            &authorized_account_value_l1,
        )
        .expect("Failed to build order_intent_utxo");

        // Build account_balance_utxo dynamically with correct address and datum
        let balance_asset = Asset {
            unit: format!(
                "{}ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e",
                scripts.hydra_token_mint.hash
            ),
            quantity: "10000000000000".to_string(),
        };
        let account_balance_utxo = build_account_balance_utxo(
            &scripts,
            "18a30e95153b789dce51c7eecd7f889e919864b828a77b8f7ac2df7696f39127",
            5,
            &user_account,
            &balance_asset,
        )
        .expect("Failed to build account_balance_utxo");

        let request = ProcessOrderRequest {
            address: "addr_test1qr77kjlsarq8wy22g4flrcznjh5lkug5mvth7qhhkewgmezwvc8hnnjzy82j5twzf8dfy5gjk04yd09t488ys9605dvq4ymc4x".to_string(),
            account: Some(account),
            collateral_utxo: Some(build_collateral_utxo(
                "268896892a4e5d6fb004b235be696a990073e4984772eac156da83772a6ce176",
                0,
                "10000000",
            )),
            order_intent_utxo: Some(order_intent_utxo),
            order_value_l1: vec![Asset {
                unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                quantity: "199999922800".to_string(),
            }],
            account_balance_utxos: Some(BalanceUtxos {
                utxos: vec![account_balance_utxo],
                updated_balance_l1: vec![Asset {
                    unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                    quantity: "9800000077200".to_string(),
                }],
            }),
            dex_order_book_utxo: Some(dex_order_book_utxo),
        };

        let config = AppConfig::new();
        let app_owner_wallet = get_app_owner_wallet();

        let result = handler(request, &app_owner_wallet, &config, &scripts).await;

        match result {
            Ok(response) => {
                println!("=== Process Order Result ===");
                println!("Tx Hash: {}", response.tx_hash);
                println!("Order UTxO Tx Index: {}", response.order_utxo_tx_index);
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

    #[test]
    fn test_process_order_handler_market_sell() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(test_process_order_case_market_sell());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn test_process_order_case_market_sell() {
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

        // Build order_intent_utxo dynamically with correct address and datum (market sell)
        let authorized_account_value_l1 = vec![Asset {
            unit: "lovelace".to_string(),
            quantity: "3850000000".to_string(),
        }];
        let order_intent_utxo = build_order_intent_utxo(
            &scripts,
            "53854bc3a8830d76b5e12f5711d307e7770c8848b0328b715b52f3805394bb49",
            0,
            "279aee26-ad29-4a00-a47a-fd2acea34e98",
            "lovelace",
            "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
            false, // is_buy = false (sell)
            890900000000,
            3850000000,
            10,
            &user_account,
            OrderType::Market,
            &authorized_account_value_l1,
        )
        .expect("Failed to build order_intent_utxo");

        // Build account_balance_utxo dynamically with correct address and datum
        // For market sell, the balance is in hydra token (lovelace equivalent)
        let balance_asset = Asset {
            unit: scripts.hydra_token_mint.hash.clone(),
            quantity: "5000000000".to_string(),
        };
        let account_balance_utxo = build_account_balance_utxo(
            &scripts,
            "18a30e95153b789dce51c7eecd7f889e919864b828a77b8f7ac2df7696f39127",
            0,
            &user_account,
            &balance_asset,
        )
        .expect("Failed to build account_balance_utxo");

        let request = ProcessOrderRequest {
            address: "addr_test1qr77kjlsarq8wy22g4flrcznjh5lkug5mvth7qhhkewgmezwvc8hnnjzy82j5twzf8dfy5gjk04yd09t488ys9605dvq4ymc4x".to_string(),
            account: Some(account),
            collateral_utxo: Some(build_collateral_utxo(
                "268896892a4e5d6fb004b235be696a990073e4984772eac156da83772a6ce176",
                0,
                "10000000",
            )),
            order_intent_utxo: Some(order_intent_utxo),
            order_value_l1: vec![Asset {
                unit: "lovelace".to_string(),
                quantity: "3850000000".to_string(),
            }],
            account_balance_utxos: Some(BalanceUtxos {
                utxos: vec![account_balance_utxo],
                updated_balance_l1: vec![Asset {
                    unit: "lovelace".to_string(),
                    quantity: "1150000000".to_string(),
                }],
            }),
            dex_order_book_utxo: Some(dex_order_book_utxo),
        };

        let config = AppConfig::new();
        let app_owner_wallet = get_app_owner_wallet();

        let result = handler(request, &app_owner_wallet, &config, &scripts).await;

        match result {
            Ok(response) => {
                println!("=== Process Order Result (Market Sell) ===");
                println!("Tx Hash: {}", response.tx_hash);
                println!("Order UTxO Tx Index: {}", response.order_utxo_tx_index);
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
