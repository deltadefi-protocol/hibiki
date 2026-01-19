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

    let order_redeemer =
        HydraOrderBookRedeemer::FillOrder(ByteString::new(&taker_order_id.replace("-", "")));
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

    Ok(FillOrderResponse {
        signed_tx,
        tx_hash,
        hydra_order_utxo_tx_index_map: hydra_order_utxo_tx_index_map.to_proto(),
        hydra_account_balance_tx_index_unit_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::scripts::ScriptCache;
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;
    use hibiki_proto::services::{
        AccountInfo, Asset, NewBalanceOutput, Order, UTxO, UtxoInput, UtxoOutput,
    };

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

        let request = FillOrderRequest {
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            account: Some(AccountInfo {
                account_id: "c33780ac-ae53-43c7-b4af-ec86a67a1843".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
                is_script_master_key: false,
                operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                is_script_operation_key: false,
            }),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "9cef8a9893540ba03991237ecb12a990281ce9f796010da62a7ceb269f49dd2f".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1vra9zdhfa8kteyr3mfe7adkf5nlh8jl5xcg9e7pcp5w9yhq5exvwh".to_string(),
                    amount: vec![Asset {
                        unit: "lovelace".to_string(),
                        quantity: "10000000".to_string(),
                    }],
                    data_hash: "".to_string(),
                    plutus_data: "".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            orders: vec![
                Order {
                    order_id: "6a103abb-b700-4af1-b09b-fc61369f5537".to_string(),
                    order_utxo: Some(UTxO {
                        input: Some(UtxoInput {
                            output_index: 0,
                            tx_hash: "0ec22e7a81c2f932bf3b9f770b413b7c81c6625fb1638e79d04166b6ecb55340".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzpjkekan7j0mk4e6a45020xlx3t2wxq2036pdpdx3ap9csx7ae4u".to_string(),
                            amount: vec![
                                Asset {
                                    unit: "lovelace".to_string(),
                                    quantity: "0".to_string(),
                                },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816".to_string(),
                                    quantity: "20000000".to_string(),
                                },
                            ],
                            data_hash: "30214198dca3d2d9a39fb161786ed321b0cb7d92d8873d5890a6d43068ec6382".to_string(),
                            plutus_data: "d8799f506a103abbb7004af1b09bfc61369f55379f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc81640ff9f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc8165820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4effd879801b00000066307a85001a01312d000ad8799fd8799f50c33780acae5343c7b4afec86a67a1843d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd87980ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    }),
                    updated_order_size: 0,
                    updated_price_times_one_tri: 438900000000,
                    updated_order_value_l1: vec![],
                },
                Order {
                    order_id: "73543582-9394-482e-9eec-2d3111a46283".to_string(),
                    order_utxo: Some(UTxO {
                        input: Some(UtxoInput {
                            output_index: 0,
                            tx_hash: "97697f0405c35329c6e8bf6c6825b26f3fb3adbaf0c6f6ab0f5c82cae2db0002".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzpjkekan7j0mk4e6a45020xlx3t2wxq2036pdpdx3ap9csx7ae4u".to_string(),
                            amount: vec![
                                Asset {
                                    unit: "lovelace".to_string(),
                                    quantity: "0".to_string(),
                                },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "8778000".to_string(),
                                },
                            ],
                            data_hash: "1775493a665e73d349b17712f50fa41158209c36c0d60bf86c2afa5835498993".to_string(),
                            plutus_data: "d8799f50735435829394482e9eec2d3111a462839f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc81640ff9f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc8165820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4effd87a801b00000066307a85001a0085f1100ad8799fd8799f50c33780acae5343c7b4afec86a67a1843d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd87980ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    }),
                    updated_order_size: 0,
                    updated_price_times_one_tri: 438900000000,
                    updated_order_value_l1: vec![],
                },
            ],
            taker_order_id: "6a103abb-b700-4af1-b09b-fc61369f5537".to_string(),
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "e38a1e300a4009b3edf5a809289ba695f51b2c2dd20429380c097738b4823f1d".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wrcdptezp2cdpn4gm0c72xljvzjgvapfnnvtsv34zuefe9q70mdxj".to_string(),
                    amount: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "6000000".to_string(),
                        },
                        Asset {
                            unit: "9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6d".to_string(),
                            quantity: "1".to_string(),
                        },
                    ],
                    data_hash: "4ff98216fe5c2378cc996f43c3d76d64e09d171ff2bf7b91438d0f5fead9fc69".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f50763201f8fd7440a3be071b1720b3b619d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581cbef30f7146f3370715356f4f88c64928d62708afec6796ddf1070b88581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            new_balance_outputs: vec![
                NewBalanceOutput {
                    account: Some(AccountInfo {
                        account_id: "c33780ac-ae53-43c7-b4af-ec86a67a1843".to_string(),
                        account_type: "spot_account".to_string(),
                        master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
                        is_script_master_key: false,
                        operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                        is_script_operation_key: false,
                    }),
                    balance_l1: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "19980000".to_string(),
                        },
                        Asset {
                            unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                            quantity: "8769222".to_string(),
                        },
                    ],
                },
                NewBalanceOutput {
                    account: Some(AccountInfo {
                        account_id: "763201f8-fd74-40a3-be07-1b1720b3b619".to_string(),
                        account_type: "spot_account".to_string(),
                        master_key: "04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66".to_string(),
                        is_script_master_key: false,
                        operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                        is_script_operation_key: false,
                    }),
                    balance_l1: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "20000".to_string(),
                        },
                        Asset {
                            unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                            quantity: "8778".to_string(),
                        },
                    ],
                },
            ],
        };

        let config = AppConfig::new();
        let scripts = ScriptCache::new();
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
}
