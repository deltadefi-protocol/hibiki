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
    log::info!("[PROCESS_ORDER] tx_hash: {}", tx_hash);
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
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;
    use hibiki_proto::services::{AccountInfo, Asset, BalanceUtxos, UTxO, UtxoInput, UtxoOutput};

    #[test]
    fn test_process_order_handler_exists() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(test_process_order_case_1());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn test_process_order_case_1() {
        init_test_env();

        let account = AccountInfo {
            account_id: "569c4c28-6389-40ac-aa30-e573f8969f09".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        let request = ProcessOrderRequest {
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            account: Some(account),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "c5a73bfd3a4b6ade924a2179c0ebe625a0b0529a2650eaabeeb83dd62b56ffd1".to_string(),
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
            order_intent_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "8b0de154d9fcdd845c5eadf66502da051d2d3fdaa3165768aa58fc253cf50e25".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wqen5pwawre7mkl4d42yr4673fgnc6awu7htu5zhx5dwshcmvju36".to_string(),
                    amount: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "0".to_string(),
                        },
                        Asset {
                            unit: "333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f".to_string(),
                            quantity: "1".to_string(),
                        },
                    ],
                    data_hash: "0e5f066a532d31b6197a44e3a13d254a1506f70262410dde097e3ea5da9b5fe3".to_string(),
                    plutus_data: "d8799fd8799fd8799f50569c4c28638940acaa30e573f8969f09d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd8799fd8799f5024ca5010686144b69fcd64e666efce899f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc81640ff9f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc8165820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4effd87a801b00000066307a85001a0085f1100ad8799fd8799f50569c4c28638940acaa30e573f8969f09d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd87980ffa1581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816a15820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e1a0085f110ffff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            order_value_l1: vec![Asset {
                unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                quantity: "8778000".to_string(),
            }],
            account_balance_utxos: Some(BalanceUtxos {
                utxos: vec![UTxO {
                    input: Some(UtxoInput {
                        output_index: 5,
                        tx_hash: "d24987dbbcdeac5726bf3734e14b5dc86d22dde305bfc4fae2c5ab1fcc0826c7".to_string(),
                    }),
                    output: Some(UtxoOutput {
                        address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                        amount: vec![
                            Asset {
                                unit: "lovelace".to_string(),
                                quantity: "0".to_string(),
                            },
                            Asset {
                                unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                quantity: "10000000000000".to_string(),
                            },
                        ],
                        data_hash: "395bb9eb71b911dff1337eed707161795d5dea6791a64a9f6825d0aca20845c5".to_string(),
                        plutus_data: "d8799fd8799f50569c4c28638940acaa30e573f8969f09d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                        script_ref: "".to_string(),
                        script_hash: "".to_string(),
                    }),
                }],
                updated_balance_l1: vec![Asset {
                    unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                    quantity: "9999991222000".to_string(),
                }],
            }),
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "23df334ce4ac85f4d6ea3468439f87ec907f9d6df8f595d1751acd0f4591ce60".to_string(),
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
                    data_hash: "db15212723f8ecfd6fbaf0a7a52ccee752f164f2dfda685fa673b4de6db3d6c7".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f5019fb5dfe07d045719104d39e9a0bf8b0d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581cbef30f7146f3370715356f4f88c64928d62708afec6796ddf1070b88581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
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
    fn test_process_order_case_2() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(run_process_order_case_2());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn run_process_order_case_2() {
        init_test_env();

        let account = AccountInfo {
            account_id: "569c4c28-6389-40ac-aa30-e573f8969f09".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        let request = ProcessOrderRequest {
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            account: Some(account),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "c5a73bfd3a4b6ade924a2179c0ebe625a0b0529a2650eaabeeb83dd62b56ffd1".to_string(),
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
            order_intent_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "3669a25930c08b7104de3a595a4e58c4ea26ec547d95c816e2b43f39569f6438".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wqen5pwawre7mkl4d42yr4673fgnc6awu7htu5zhx5dwshcmvju36".to_string(),
                    amount: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "0".to_string(),
                        },
                        Asset {
                            unit: "333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f".to_string(),
                            quantity: "1".to_string(),
                        },
                    ],
                    data_hash: "a9868c6508fa9b3f49e2e6d3464f184be27ef09504b6ed0fced5259b589496f2".to_string(),
                    plutus_data: "d8799fd8799fd8799f50569c4c28638940acaa30e573f8969f09d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd8799fd8799f503f00e2569c3e4e3186f585b3dffc62219f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc81640ff9f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc8165820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4effd87a801b00000066307a85001a0085f1100ad8799fd8799f50569c4c28638940acaa30e573f8969f09d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd87980ffa1581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816a15820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e1a0085f110ffff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            order_value_l1: vec![Asset {
                unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                quantity: "8778000".to_string(),
            }],
            account_balance_utxos: Some(BalanceUtxos {
                utxos: vec![UTxO {
                    input: Some(UtxoInput {
                        output_index: 5,
                        tx_hash: "d24987dbbcdeac5726bf3734e14b5dc86d22dde305bfc4fae2c5ab1fcc0826c7".to_string(),
                    }),
                    output: Some(UtxoOutput {
                        address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                        amount: vec![
                            Asset {
                                unit: "lovelace".to_string(),
                                quantity: "0".to_string(),
                            },
                            Asset {
                                unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                quantity: "10000000000000".to_string(),
                            },
                        ],
                        data_hash: "395bb9eb71b911dff1337eed707161795d5dea6791a64a9f6825d0aca20845c5".to_string(),
                        plutus_data: "d8799fd8799f50569c4c28638940acaa30e573f8969f09d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                        script_ref: "".to_string(),
                        script_hash: "".to_string(),
                    }),
                }],
                updated_balance_l1: vec![Asset {
                    unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                    quantity: "9999991222000".to_string(),
                }],
            }),
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "23df334ce4ac85f4d6ea3468439f87ec907f9d6df8f595d1751acd0f4591ce60".to_string(),
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
                    data_hash: "db15212723f8ecfd6fbaf0a7a52ccee752f164f2dfda685fa673b4de6db3d6c7".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f5019fb5dfe07d045719104d39e9a0bf8b0d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581cbef30f7146f3370715356f4f88c64928d62708afec6796ddf1070b88581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
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
                println!("=== Process Order Case 2 Result ===");
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
    fn test_process_order_case_3() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(run_process_order_case_3());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn run_process_order_case_3() {
        init_test_env();

        let account = AccountInfo {
            account_id: "fb97a6ce-361e-4063-b8b4-0da7545e1ee9".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        let request = ProcessOrderRequest {
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            account: Some(account),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "7484ea8cca646ad42a357f6d5973563ce1914badf0a93e7acb0b41d4b136c440".to_string(),
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
            order_intent_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "8c69b7286a9bcedd141f497635da91ec0fd2604c3e11641a8c845b03806f3cd6".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wqen5pwawre7mkl4d42yr4673fgnc6awu7htu5zhx5dwshcmvju36".to_string(),
                    amount: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "0".to_string(),
                        },
                        Asset {
                            unit: "333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f".to_string(),
                            quantity: "1".to_string(),
                        },
                    ],
                    data_hash: "9cdb854df1d8aed4c9094239ccb207f022d6d1b6c9dca222930cbc2c83edb960".to_string(),
                    plutus_data: "d8799fd8799fd8799f50fb97a6ce361e4063b8b40da7545e1ee9d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd8799fd8799f50a5b813e8987d4594b16b80e95964686f9f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc81640ff9f581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc8165820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4effd87a801b00000066307a85001a006474cc0ad8799fd8799f50fb97a6ce361e4063b8b40da7545e1ee9d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd87980ffa1581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816a15820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e1a006474ccffff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            order_value_l1: vec![Asset {
                unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                quantity: "6583500".to_string(),
            }],
            account_balance_utxos: Some(BalanceUtxos {
                utxos: vec![UTxO {
                    input: Some(UtxoInput {
                        output_index: 1,
                        tx_hash: "d9cba8ef3036d01aa9216aaa21f0c9b6dcc1a064c5e8a8e0e317d4cfae2d0ee6".to_string(),
                    }),
                    output: Some(UtxoOutput {
                        address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                        amount: vec![
                            Asset {
                                unit: "lovelace".to_string(),
                                quantity: "0".to_string(),
                            },
                            Asset {
                                unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                quantity: "8769222".to_string(),
                            },
                        ],
                        data_hash: "f147d0fe08731ceb3442ed57d886597d086a406571e3afa7eee6964ddaf81bfa".to_string(),
                        plutus_data: "d8799fd8799f50fb97a6ce361e4063b8b40da7545e1ee9d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                        script_ref: "".to_string(),
                        script_hash: "".to_string(),
                    }),
                }],
                updated_balance_l1: vec![Asset {
                    unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                    quantity: "2185722".to_string(),
                }],
            }),
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "0db4a1a918d1c3d0ddf7308542d43c1cb0e57420a4f75a4db21dbfe22ac22c3a".to_string(),
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
                    data_hash: "45150b1d8addbc0f37660b75235b6a305ae1bf236087a9263de3b77d60efd140".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f504f6ca5a7f28b45e7949d556740c69c0cd8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581cbef30f7146f3370715356f4f88c64928d62708afec6796ddf1070b88581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
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
                println!("=== Process Order Case 3 Result ===");
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
