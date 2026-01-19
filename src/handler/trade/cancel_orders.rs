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
    }

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

    Ok(CancelOrdersResponse {
        signed_tx,
        tx_hash,
        account_utxo_tx_index_unit_map: unit_tx_index_map.to_proto(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::scripts::ScriptCache;
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;
    use hibiki_proto::services::{AccountInfo, Asset, Order, UTxO, UtxoInput, UtxoOutput};

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

        let account = AccountInfo {
            account_id: "68bbb95a-6982-4f2a-9207-9906d4743815".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        let request = CancelOrdersRequest {
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            account: Some(account),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "5606ae3bcfcdee7a3658bdbd499e2201e43e67194cbe4b30d684b9a0dc08f367".to_string(),
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
            new_balance_l1: vec![Asset {
                unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                quantity: "8778000".to_string(),
            }],
            orders: vec![Order {
                order_id: "646ba6a5-c6d0-4605-832d-c78f2cbcfc5e".to_string(),
                order_utxo: Some(UTxO {
                    input: Some(UtxoInput {
                        output_index: 0,
                        tx_hash: "2182b52fa84199bc876dd91cc5e99f919b028426d16af33bf4ae2513bb814b67".to_string(),
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
                        data_hash: "7a0cfb7988bef5918e186ebd1418475e0a02b0ba62e5a93748105e3eab833063".to_string(),
                        plutus_data: "d8799f50646ba6a5c6d04605832dc78f2cbcfc5e9f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc81640ff9f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc8165820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4effd87a801b00000066307a85001a0085f1100ad8799fd8799f5068bbb95a69824f2a92079906d4743815d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd87980ff".to_string(),
                        script_ref: "".to_string(),
                        script_hash: "".to_string(),
                    }),
                }),
                updated_order_size: 0,
                updated_price_times_one_tri: 0,
                updated_order_value_l1: vec![],
            }],
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "de92cf2d3e22f033eab435b29e4457dd23fc086ea1955913d3a65ffe2272e95d".to_string(),
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
                    data_hash: "16db19bfaf6d1c5c124b2c17d1dd62f06e9ddd765474514bff6f9dbe390318f4".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f50c29f54fdb91a4cbeb05208776539aaf2d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581cbef30f7146f3370715356f4f88c64928d62708afec6796ddf1070b88581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
        };

        let config = AppConfig::new();
        let scripts = ScriptCache::new();
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
