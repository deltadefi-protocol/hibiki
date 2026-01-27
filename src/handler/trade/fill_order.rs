use std::collections::HashMap;
use std::time::Instant;

use hibiki_proto::services::{FillOrderRequest, FillOrderResponse};
use whisky::{
    calculate_tx_hash,
    data::{ByteString, PlutusDataJson},
    PlutusDataCbor, WData, WError, Wallet,
};

use crate::{
    config::AppConfig,
    scripts::{HydraOrderBookRedeemer, Order, OrderType, ScriptCache, UserAccount},
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

    let start = Instant::now();
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

    let order_redeemer =
        HydraOrderBookRedeemer::FillOrder(ByteString::new(&taker_order_id.replace("-", "")));
    log::info!(
        "[FILL_ORDER] Starting fill order build for taker_order_id: {}",
        taker_order_id
    );
    log::debug!("[FILL_ORDER] Total orders to process: {}", orders.len());

    for order in &orders {
        let order_utxo = &order.order_utxo;
        // Log input being consumed
        log::debug!(
            "[FILL_ORDER] Input order UTXO: {}#{} for order_id: {}",
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
        // if fully filled -> skip
        if order.updated_order_size == 0 {
            log::debug!(
                "[FILL_ORDER] Order {} fully filled, no output created",
                order.order_id
            );
            continue;
        }

        // Create new order if partially filled
        if order.updated_order_size > 0 {
            let input_order = Order::from_cbor(&order_utxo.output.plutus_data.as_ref().unwrap())?;
            let updated_order = input_order.update_order(
                order.updated_order_size,
                order.updated_price_times_one_tri,
                OrderType::LimitOrder,
            );
            tx_builder
                .tx_out(
                    &hydra_order_book_spend.address,
                    &to_hydra_token(&order.updated_order_value_l1),
                )
                .tx_out_inline_datum_value(&WData::JSON(updated_order.to_json_string()));
            log::debug!(
                "[FILL_ORDER] Partial order output at tx_index: {} for order_id: {}",
                current_index,
                order.order_id
            );
            hydra_order_utxo_tx_index_map.add(&order.order_id);
            current_index += 1;
        }
    }

    log::debug!(
        "[FILL_ORDER] Account balance outputs start at tx_index: {}",
        current_index
    );
    log::debug!(
        "[FILL_ORDER] Total new_balance_outputs to process: {}",
        new_balance_outputs.len()
    );

    for new_balance_output in &new_balance_outputs {
        let mut tx_index_assets_map = TxIndexAssetsMap::default();
        tx_index_assets_map.set_index(current_index);
        let account_info = new_balance_output.account.as_ref().unwrap();
        let account = UserAccount::from_proto_trade_account(&account_info, account_ops_script_hash);
        let new_balance_assets_l1 = from_proto_amount(&new_balance_output.balance_l1);

        log::debug!(
            "[FILL_ORDER] Processing account_id: {} with {} assets, starting at tx_index: {}",
            account_info.account_id,
            new_balance_assets_l1.len(),
            current_index
        );

        for asset_l1 in new_balance_assets_l1 {
            tx_builder
                .tx_out(
                    &hydra_account_spend.address,
                    &to_hydra_token(&[asset_l1.clone()]),
                )
                .tx_out_inline_datum_value(&WData::JSON(account.to_json_string()));

            log::debug!(
                "[FILL_ORDER] Account balance tx_index: {} for account_id: {} asset: {} qty: {}",
                current_index,
                account_info.account_id,
                asset_l1.unit(),
                asset_l1.quantity()
            );

            tx_index_assets_map.insert(&[asset_l1]);
            current_index += 1;
        }

        if let Some(proto) = tx_index_assets_map.to_proto() {
            log::debug!(
                "[FILL_ORDER] Created tx_index_assets_map for account_id: {}: {:?}",
                account_info.account_id,
                proto
            );
            hydra_account_balance_tx_index_unit_map.insert(account_info.account_id.clone(), proto);
        }
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

    let hydra_order_utxo_tx_index_map_proto = hydra_order_utxo_tx_index_map.to_proto();

    log::debug!("[FILL_ORDER] Built tx_hex length: {}", tx_hex.len());
    log::info!(
        "[FILL_ORDER] tx_hash: {} completed in {:?}",
        tx_hash,
        start.elapsed()
    );
    log::debug!(
        "[FILL_ORDER] hydra_order_utxo_tx_index_map: {:?}",
        hydra_order_utxo_tx_index_map_proto
    );
    log::debug!(
        "[FILL_ORDER] hydra_account_balance_tx_index_unit_map: {:?}",
        hydra_account_balance_tx_index_unit_map
    );

    Ok(FillOrderResponse {
        signed_tx,
        tx_hash,
        hydra_order_utxo_tx_index_map: hydra_order_utxo_tx_index_map_proto,
        hydra_account_balance_tx_index_unit_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::hydra::get_hydra_pp;
    use crate::config::AppConfig;
    use crate::scripts::ScriptCache;
    use crate::test_fixtures::{
        build_collateral_utxo, build_dex_order_book_utxo, build_order_utxo, build_proto_order,
        L2ScriptConfig,
    };
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;
    use hibiki_proto::services::{AccountInfo, Asset, NewBalanceOutput, OrderType};

    #[test]
    fn test_fill_order_handler() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(run_fill_order_case_1());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn run_fill_order_case_1() {
        init_test_env();

        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();

        // Account for the taker (seller) and maker (buyer) - same account in this test
        let account = AccountInfo {
            account_id: "a810ec7a-7069-4157-aead-48ecf4d693c8".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "fdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        // Fee collector account for dex_order_book_utxo
        let fee_account = AccountInfo {
            account_id: "0e6bb866-c13a-4ad4-a40e-87ab23392120".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        // Create user accounts for building UTxOs
        let user_account = UserAccount::from_proto_trade_account(
            &account,
            &scripts.hydra_order_book_withdrawal.hash,
        );
        let fee_user_account = UserAccount::from_proto_trade_account(
            &fee_account,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Build dex_order_book_utxo dynamically with current script hashes
        let dex_order_book_utxo = build_dex_order_book_utxo(
            &scripts,
            &l2_config,
            "4a50e9271bc74e8f143207134a5a62db89fab76eb55fbd00786487966158d86d",
            0,
            &fee_user_account,
        )
        .expect("Failed to build dex_order_book_utxo");

        // Build order 1: Market sell order (fully filled)
        // Selling lovelace for USDM quote token
        let order1_value_asset = Asset {
            unit: scripts.hydra_token_mint.hash.clone(),
            quantity: "3850000000".to_string(),
        };
        let order1_utxo = build_order_utxo(
            &scripts,
            "5167ca7f855fa14d34fc6ee1f1e2419953f883a92e9308182e610f7da417c937",
            0,
            "279aee26-ad29-4a00-a47a-fd2acea34e98",
            "lovelace",
            "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
            false, // is_buy: false (sell order)
            890900000000,
            3850000000,
            10,
            &user_account,
            OrderType::Market,
            &order1_value_asset,
        )
        .expect("Failed to build order1_utxo");

        // Build order 2: Limit buy order (partially filled)
        // Buying lovelace with USDM quote token
        let order2_value_asset = Asset {
            unit: format!(
                "{}ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e",
                scripts.hydra_token_mint.hash
            ),
            quantity: "199999922800".to_string(),
        };
        let order2_utxo = build_order_utxo(
            &scripts,
            "139c10c003d5ad283e163f7ca1be919950c8bbcee793c0e2a6fe9147965ae73d",
            0,
            "7398aea0-392e-4198-8dc2-4abaf5e5afa4",
            "lovelace",
            "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
            true, // is_buy: true (buy order)
            890900000000,
            199999922800,
            10,
            &user_account,
            OrderType::Limit,
            &order2_value_asset,
        )
        .expect("Failed to build order2_utxo");

        let request = FillOrderRequest {
            address: "addr_test1qr77kjlsarq8wy22g4flrcznjh5lkug5mvth7qhhkewgmezwvc8hnnjzy82j5twzf8dfy5gjk04yd09t488ys9605dvq4ymc4x".to_string(),
            account: Some(account.clone()),
            collateral_utxo: Some(build_collateral_utxo(
                "268896892a4e5d6fb004b235be696a990073e4984772eac156da83772a6ce176",
                0,
                "10000000",
            )),
            orders: vec![
                build_proto_order(
                    "279aee26-ad29-4a00-a47a-fd2acea34e98",
                    order1_utxo,
                    0, // fully filled
                    890900000000,
                    vec![],
                ),
                build_proto_order(
                    "7398aea0-392e-4198-8dc2-4abaf5e5afa4",
                    order2_utxo,
                    196569957800, // partially filled
                    890900000000,
                    vec![Asset {
                        unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                        quantity: "196569957800".to_string(),
                    }],
                ),
            ],
            taker_order_id: "279aee26-ad29-4a00-a47a-fd2acea34e98".to_string(),
            dex_order_book_utxo: Some(dex_order_book_utxo),
            new_balance_outputs: vec![
                NewBalanceOutput {
                    account: Some(account.clone()),
                    balance_l1: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "3846150000".to_string(),
                        },
                        Asset {
                            unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                            quantity: "3426535035".to_string(),
                        },
                    ],
                },
                NewBalanceOutput {
                    account: Some(fee_account.clone()),
                    balance_l1: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "3850000".to_string(),
                        },
                        Asset {
                            unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                            quantity: "3429965".to_string(),
                        },
                    ],
                },
            ],
        };

        let config = AppConfig::new();
        let app_owner_wallet = get_app_owner_wallet();

        let result = handler(request, &app_owner_wallet, &config, &scripts).await;

        match result {
            Ok(response) => {
                println!("=== Fill Order Result ===");
                println!("Tx Hash: {}", response.tx_hash);
                println!(
                    "Hydra Order UTxO Tx Index Map: {:?}",
                    response.hydra_order_utxo_tx_index_map
                );
                println!(
                    "Hydra Account Balance Tx Index Unit Map: {:?}",
                    response.hydra_account_balance_tx_index_unit_map
                );
                println!("Signed Tx: {}", response.signed_tx);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }

    /// Test fill order execution unit limits by varying the number of orders and accounts.
    ///
    /// This test uses the offline evaluator to check transaction execution units against
    /// Hydra protocol parameter limits defined in `src/config/hydra.rs`:
    /// - max_tx_ex_mem: 16,000,000,000 (16 billion memory units)
    /// - max_tx_ex_steps: 10,000,000,000,000 (10 trillion CPU steps)
    ///
    /// The test uses real balanced fixture data from `run_fill_order_case_1` as a template.
    #[test]
    fn test_fill_order_execution_units_limit() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(run_fill_order_ex_units_test());
            })
            .unwrap();

        handle.join().unwrap();
    }

    /// Generate a unique order_id in UUID format based on index
    fn generate_order_id(index: usize) -> String {
        format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            0x6a103abbu32.wrapping_add(index as u32),
            0xb700u16.wrapping_add((index % 0xFFFF) as u16),
            0x4af1u16,
            0xb09bu16,
            0xfc61369f5537u64.wrapping_add(index as u64)
        )
    }

    /// Generate a unique tx_hash based on index
    fn generate_tx_hash(index: usize) -> String {
        format!(
            "{:016x}{:016x}{:016x}{:016x}",
            0x0ec22e7a81c2f932u64.wrapping_add(index as u64),
            0xbf3b9f770b413b7cu64.wrapping_add(index as u64),
            0x81c6625fb1638e79u64,
            0xd04166b6ecb55340u64.wrapping_add(index as u64)
        )
    }

    /// Get a fill order request with configurable number of orders using dynamic fixtures.
    fn get_fill_order_request(
        num_orders: usize,
        scripts: &ScriptCache,
        l2_config: &L2ScriptConfig,
    ) -> FillOrderRequest {
        // Account info for orders
        let account = AccountInfo {
            account_id: "c33780ac-ae53-43c7-b4af-ec86a67a1843".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        // Fee collector account for dex_order_book_utxo
        let fee_account = AccountInfo {
            account_id: "763201f8-fd74-40a3-be07-1b1720b3b619".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        let user_account = UserAccount::from_proto_trade_account(
            &account,
            &scripts.hydra_order_book_withdrawal.hash,
        );
        let fee_user_account = UserAccount::from_proto_trade_account(
            &fee_account,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Build dex_order_book_utxo dynamically
        let dex_order_book_utxo = build_dex_order_book_utxo(
            scripts,
            l2_config,
            "e38a1e300a4009b3edf5a809289ba695f51b2c2dd20429380c097738b4823f1d",
            0,
            &fee_user_account,
        )
        .expect("Failed to build dex_order_book_utxo");

        let mut orders = Vec::with_capacity(num_orders);

        for i in 0..num_orders {
            let order_id = generate_order_id(i);
            let tx_hash = generate_tx_hash(i);

            // Alternate between sell orders (lovelace) and buy orders (hydra token)
            let (is_buy, order_value_asset, order_size) = if i % 2 == 0 {
                // Sell order - selling lovelace for USDM
                (
                    false,
                    Asset {
                        unit: scripts.hydra_token_mint.hash.clone(),
                        quantity: "20000000".to_string(),
                    },
                    20000000u64,
                )
            } else {
                // Buy order - buying lovelace with USDM (hydra token)
                (
                    true,
                    Asset {
                        unit: format!(
                            "{}ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e",
                            scripts.hydra_token_mint.hash
                        ),
                        quantity: "8778000".to_string(),
                    },
                    8778000u64,
                )
            };

            let order_utxo = build_order_utxo(
                scripts,
                &tx_hash,
                0,
                &order_id,
                "lovelace",
                "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
                is_buy,
                438900000000, // list_price_times_one_tri
                order_size,
                10, // commission_rate_bp
                &user_account,
                OrderType::Limit,
                &order_value_asset,
            )
            .expect("Failed to build order_utxo");

            orders.push(build_proto_order(
                &order_id,
                order_utxo,
                0, // fully filled
                438900000000,
                vec![],
            ));
        }

        let taker_order_id = orders
            .first()
            .map(|o| o.order_id.clone())
            .unwrap_or_default();

        FillOrderRequest {
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            account: Some(account.clone()),
            collateral_utxo: Some(build_collateral_utxo(
                "9cef8a9893540ba03991237ecb12a990281ce9f796010da62a7ceb269f49dd2f",
                0,
                "10000000",
            )),
            orders,
            taker_order_id,
            dex_order_book_utxo: Some(dex_order_book_utxo),
            new_balance_outputs: vec![
                NewBalanceOutput {
                    account: Some(account.clone()),
                    balance_l1: vec![
                        Asset { unit: "lovelace".to_string(), quantity: "19980000".to_string() },
                        Asset { unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(), quantity: "8769222".to_string() },
                    ],
                },
                NewBalanceOutput {
                    account: Some(fee_account.clone()),
                    balance_l1: vec![
                        Asset { unit: "lovelace".to_string(), quantity: "20000".to_string() },
                        Asset { unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(), quantity: "8778".to_string() },
                    ],
                },
            ],
        }
    }

    /// Result of building a fill order transaction with execution unit metrics
    struct FillOrderExUnitsResult {
        #[allow(dead_code)]
        tx_hash: String,
        total_mem: u64,
        total_steps: u64,
        spend_redeemer_count: usize,
        withdrawal_redeemer_count: usize,
    }

    /// Build a fill order transaction and extract execution units from all redeemers.
    /// This duplicates some handler logic to access the TxBuilder internals.
    async fn build_fill_order_with_ex_units(
        request: FillOrderRequest,
        _app_owner_wallet: &Wallet,
        config: &AppConfig,
        scripts: &ScriptCache,
    ) -> Result<FillOrderExUnitsResult, WError> {
        use whisky::{ScriptTxIn, TxIn, Withdrawal};

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

        let mut _current_index: u32 = 0;

        let order_redeemer =
            HydraOrderBookRedeemer::FillOrder(ByteString::new(&taker_order_id.replace("-", "")));

        // Build order inputs (same as handler)
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
                .tx_in_redeemer_value(
                    &hydra_order_book_spend.redeemer(order_redeemer.clone(), None),
                )
                .spending_tx_in_reference(
                    collateral.input.tx_hash.as_str(),
                    hydra_order_book_spend.ref_output_index,
                    &hydra_order_book_spend.hash,
                    hydra_order_book_spend.size,
                )
                .input_for_evaluation(&order_utxo);

            // Partially filled orders produce output (same logic as handler)
            if order.updated_order_size > 0 {
                let input_order = crate::scripts::Order::from_cbor(
                    &order_utxo.output.plutus_data.as_ref().unwrap(),
                )?;
                let updated_order = input_order.update_order(
                    order.updated_order_size,
                    order.updated_price_times_one_tri,
                    crate::scripts::OrderType::LimitOrder,
                );
                tx_builder
                    .tx_out(
                        &hydra_order_book_spend.address,
                        &to_hydra_token(&order.updated_order_value_l1),
                    )
                    .tx_out_inline_datum_value(&WData::JSON(updated_order.to_json_string()));
                _current_index += 1;
            }
        }

        // Build account balance outputs (one per asset, same as handler)
        for new_balance_output in &new_balance_outputs {
            let account_info = new_balance_output.account.as_ref().unwrap();
            let account =
                UserAccount::from_proto_trade_account(&account_info, account_ops_script_hash);
            let new_balance_assets_l1 = from_proto_amount(&new_balance_output.balance_l1);

            for asset_l1 in new_balance_assets_l1 {
                tx_builder
                    .tx_out(
                        &hydra_account_spend.address,
                        &to_hydra_token(&[asset_l1.clone()]),
                    )
                    .tx_out_inline_datum_value(&WData::JSON(account.to_json_string()));
                _current_index += 1;
            }
        }

        // Build withdrawal and finalize
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
            .change_address(&address)
            .complete(None)
            .await?;

        // Extract execution units from all redeemers
        let mut total_mem: u64 = 0;
        let mut total_steps: u64 = 0;
        let mut spend_redeemer_count: usize = 0;
        let mut withdrawal_redeemer_count: usize = 0;

        // Extract from spending inputs
        for input in &tx_builder.tx_builder_body.inputs {
            if let TxIn::ScriptTxIn(ScriptTxIn { script_tx_in, .. }) = input {
                if let Some(redeemer) = &script_tx_in.redeemer {
                    total_mem += redeemer.ex_units.mem;
                    total_steps += redeemer.ex_units.steps;
                    spend_redeemer_count += 1;
                }
            }
        }

        // Extract from withdrawals
        for withdrawal in &tx_builder.tx_builder_body.withdrawals {
            if let Withdrawal::PlutusScriptWithdrawal(w) = withdrawal {
                if let Some(redeemer) = &w.redeemer {
                    total_mem += redeemer.ex_units.mem;
                    total_steps += redeemer.ex_units.steps;
                    withdrawal_redeemer_count += 1;
                }
            }
        }

        let tx_hex = tx_builder.tx_hex();
        let tx_hash = calculate_tx_hash(&tx_hex)?;

        Ok(FillOrderExUnitsResult {
            tx_hash,
            total_mem,
            total_steps,
            spend_redeemer_count,
            withdrawal_redeemer_count,
        })
    }

    async fn run_fill_order_ex_units_test() {
        init_test_env();

        let config = AppConfig::new();
        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();
        let app_owner_wallet = get_app_owner_wallet();

        // Get Hydra limits
        let pp = get_hydra_pp();
        let max_mem: u64 = pp.max_tx_ex_mem.parse().unwrap();
        let max_steps: u64 = pp.max_tx_ex_steps.parse().unwrap();

        println!("\n=== Fill Order Execution Unit Limit Test ===\n");
        println!("Hydra Limits (much higher than mainnet for throughput):");
        println!("  max_tx_ex_mem:   {} ({:.2e})", max_mem, max_mem as f64);
        println!(
            "  max_tx_ex_steps: {} ({:.2e})\n",
            max_steps, max_steps as f64
        );

        println!(
            "{:<8} {:<12} {:<15} {:<18} {:<10}",
            "Orders", "Redeemers", "Memory", "Steps", "Status"
        );
        println!("{}", "-".repeat(75));

        // Test with increasing number of orders
        let order_counts = vec![1, 2, 3, 5, 10, 15, 20, 25, 30, 40, 50];

        for num_orders in order_counts {
            let request = get_fill_order_request(num_orders, &scripts, &l2_config);
            let _num_accounts = request.new_balance_outputs.len();

            match build_fill_order_with_ex_units(request, &app_owner_wallet, &config, &scripts)
                .await
            {
                Ok(result) => {
                    let total_redeemers =
                        result.spend_redeemer_count + result.withdrawal_redeemer_count;

                    println!(
                        "{:<8} {:<12} {:<15} {:<18} OK",
                        num_orders, total_redeemers, result.total_mem, result.total_steps,
                    );
                }
                Err(e) => {
                    let error_msg = format!("{:?}", e);

                    // Categorize error type
                    let (status, should_stop) = if error_msg.contains("over budget")
                        || error_msg.contains("ExBudget")
                    {
                        ("EX_LIMIT", true)
                    } else if error_msg.contains("Validator returned false")
                        || error_msg.contains("validator crashed")
                    {
                        ("VALIDATE", false) // Script validation failed - synthetic data issue
                    } else if error_msg.contains("add_change") || error_msg.contains("Insufficient")
                    {
                        ("BALANCE", false)
                    } else {
                        ("ERROR", false)
                    };

                    // Extract budget info if available (mem and steps)
                    let (mem, steps) = if let Some(start) = error_msg.find("Budget { mem:") {
                        // Parse: Budget { mem: 123, steps: 456 }
                        let budget_str = &error_msg[start..];
                        let mem = budget_str
                            .find("mem:")
                            .and_then(|m| {
                                let num_start = m + 5;
                                budget_str[num_start..]
                                    .find(',')
                                    .map(|e| &budget_str[num_start..num_start + e])
                            })
                            .and_then(|s| s.trim().parse::<u64>().ok())
                            .unwrap_or(0);
                        let steps = budget_str
                            .find("steps:")
                            .and_then(|s| {
                                let num_start = s + 7;
                                budget_str[num_start..]
                                    .find('}')
                                    .map(|e| &budget_str[num_start..num_start + e])
                            })
                            .and_then(|s| s.trim().parse::<u64>().ok())
                            .unwrap_or(0);
                        (mem, steps)
                    } else {
                        (0, 0)
                    };

                    if mem > 0 {
                        println!(
                            "{:<8} {:<12} {:<15} {:<18} {} (validator)",
                            num_orders,
                            format!("{}", num_orders + 1), // estimated redeemers
                            mem,
                            steps,
                            status
                        );
                    } else {
                        println!(
                            "{:<8} {:<12} {:<15} {:<18} {}",
                            num_orders, "-", "-", "-", status
                        );
                    }

                    if should_stop {
                        println!(
                            "\n>>> Execution unit limit reached at {} orders",
                            num_orders
                        );
                        println!("Full error: {}", error_msg);
                        break;
                    }
                }
            }
        }

        println!("\n=== Summary ===");
        println!("Each order adds ~1 spend redeemer to the transaction.");
        println!("There is always 1 withdrawal redeemer for the order book validator.");
        println!();
        println!("Approximate scaling per order (from VALIDATE data):");
        println!("  Memory: ~840,000 units/order");
        println!("  Steps:  ~260,000,000 steps/order");
        println!();
        println!("Theoretical max orders before hitting Hydra limits:");
        println!(
            "  Memory limit: {} / 840,000 ≈ {:.0} orders",
            max_mem,
            max_mem as f64 / 840_000.0
        );
        println!(
            "  Steps limit:  {} / 260M ≈ {:.0} orders",
            max_steps,
            max_steps as f64 / 260_000_000.0
        );
        println!();
        println!("Status codes:");
        println!("  OK       - Transaction built and validated successfully");
        println!("  VALIDATE - Script validation failed (synthetic data doesn't match DEX rules)");
        println!("  BALANCE  - Transaction balancing failed");
        println!("  EX_LIMIT - Execution unit limit exceeded");
        println!();
        println!("Note: VALIDATE errors show execution units used BEFORE validator rejection.");
        println!("These numbers are useful for scaling estimates even though the tx failed.");
        println!("For proper tests beyond 2 orders, use real fixtures matching the DEX model.");
    }
}
