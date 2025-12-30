use hibiki_proto::services::{
    AssetList, ProcessTransferRequest, ProcessTransferResponse, UnitTxIndexMap,
};
use std::collections::HashMap;
use whisky::{
    calculate_tx_hash,
    data::{Constr, List, PlutusData, PlutusDataJson},
    Budget, UTxO, UtxoInput, UtxoOutput, WData, WError, WRedeemer, Wallet,
};

use crate::{
    config::AppConfig,
    constant::{all_hydra_to_l1_token_map, dex_oracle_nft, l2_ref_scripts_index},
    handler::sign_transaction::check_signature_sign_tx,
    scripts::{
        hydra_account_spend_spending_blueprint, hydra_account_withdraw_withdrawal_blueprint,
        hydra_user_intent_mint_minting_blueprint, hydra_user_intent_spend_spending_blueprint,
        HydraAccountOperation, HydraAccountRedeemer, HydraUserIntentRedeemer, UserTradeAccount,
    },
    utils::{
        hydra::{get_hydra_tx_builder, get_script_ref_hex},
        proto::{
            extract_transfer_amount_from_intent, from_proto_balance_utxos, from_proto_utxo,
            to_proto_amount,
        },
        token::{to_hydra_token, to_l1_assets},
    },
};

pub async fn handler(
    request: ProcessTransferRequest,
    app_owner_wallet: &Wallet,
) -> Result<ProcessTransferResponse, WError> {
    let AppConfig { app_owner_vkey, .. } = AppConfig::new();

    let collateral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(request.dex_order_book_utxo.as_ref().unwrap());
    let intent_utxo = from_proto_utxo(request.transferral_intent_utxo.as_ref().unwrap());
    let from_account = UserTradeAccount::from_proto(request.account.as_ref().unwrap());
    let to_account = UserTradeAccount::from_proto(request.receiver_account.as_ref().unwrap());

    // Parse sender's balance UTXOs
    let (from_updated_balance_l1, from_account_utxos) =
        from_proto_balance_utxos(request.account_balance_utxos.as_ref().unwrap());

    let to_updated_balance_l2 = extract_transfer_amount_from_intent(&intent_utxo)?;

    // For outputs, we need one UTXO address - use the first sender's UTXO address as template
    let account_balance_address = &from_account_utxos[0].output.address;

    let policy_id = whisky::data::PolicyId::new(dex_oracle_nft());
    let user_intent_mint = hydra_user_intent_mint_minting_blueprint(&policy_id);
    let user_intent_spend = hydra_user_intent_spend_spending_blueprint(&policy_id);
    let account_balance_spend = hydra_account_spend_spending_blueprint(&policy_id);
    let internal_transfer_withdraw = hydra_account_withdraw_withdrawal_blueprint(&policy_id);

    let mut from_unit_tx_index_map: HashMap<String, AssetList> =
        HashMap::with_capacity(from_updated_balance_l1.len());
    let mut to_unit_tx_index_map: HashMap<String, AssetList> =
        HashMap::with_capacity(to_updated_balance_l2.len());

    let mut current_index = 0u32;

    let intent_mint_script_ref_hex = Some(get_script_ref_hex(&user_intent_mint.cbor)?);
    let intent_mint_ref_utxo = UTxO {
        input: UtxoInput {
            output_index: l2_ref_scripts_index::hydra_user_intent::MINT,
            tx_hash: collateral.input.tx_hash.clone(),
        },
        output: UtxoOutput {
            address: collateral.output.address.clone(),
            amount: Vec::new(),
            data_hash: None,
            plutus_data: None,
            script_ref: intent_mint_script_ref_hex,
            script_hash: Some(user_intent_mint.hash.clone()),
        },
    };

    let intent_spend_script_ref_hex = Some(get_script_ref_hex(&user_intent_spend.cbor)?);
    let intent_spend_ref_utxo = UTxO {
        input: UtxoInput {
            output_index: l2_ref_scripts_index::hydra_user_intent::SPEND,
            tx_hash: collateral.input.tx_hash.clone(),
        },
        output: UtxoOutput {
            address: collateral.output.address.clone(),
            amount: Vec::new(),
            data_hash: None,
            plutus_data: None,
            script_ref: intent_spend_script_ref_hex,
            script_hash: Some(user_intent_mint.hash.clone()),
        },
    };

    let account_balance_spend_script_ref_hex =
        Some(get_script_ref_hex(&account_balance_spend.cbor)?);
    let account_balance_spend_ref_utxo = UTxO {
        input: UtxoInput {
            output_index: l2_ref_scripts_index::hydra_account_balance::SPEND,
            tx_hash: collateral.input.tx_hash.clone(),
        },
        output: UtxoOutput {
            address: collateral.output.address.clone(),
            amount: Vec::new(),
            data_hash: None,
            plutus_data: None,
            script_ref: account_balance_spend_script_ref_hex,
            script_hash: Some(user_intent_mint.hash.clone()),
        },
    };

    let account_balance_withdrawal_script_ref_hex =
        Some(get_script_ref_hex(&account_balance_spend.cbor)?);
    let account_balance_withdrawal_ref_utxo = UTxO {
        input: UtxoInput {
            output_index: l2_ref_scripts_index::hydra_account_balance::WITHDRAWAL,
            tx_hash: collateral.input.tx_hash.clone(),
        },
        output: UtxoOutput {
            address: collateral.output.address.clone(),
            amount: Vec::new(),
            data_hash: None,
            plutus_data: None,
            script_ref: account_balance_withdrawal_script_ref_hex,
            script_hash: Some(user_intent_mint.hash.clone()),
        },
    };

    let mut tx_builder = get_hydra_tx_builder();
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
        .tx_in_redeemer_value(&WRedeemer {
            data: user_intent_spend.redeemer(PlutusData::Constr(Constr::new(
                1,
                Box::new(PlutusData::List(List::new(&[]))),
            ))),
            ex_units: Budget::default(),
        })
        .spending_tx_in_reference(
            collateral.input.tx_hash.as_str(),
            l2_ref_scripts_index::hydra_user_intent::SPEND,
            &user_intent_mint.hash,
            user_intent_mint.cbor.len() / 2,
        )
        .input_for_evaluation(&intent_spend_ref_utxo)
        .input_for_evaluation(&intent_utxo)
        .input_for_evaluation(&account_balance_spend_ref_utxo);

    // Spend all sender's account balance UTXOs
    for from_utxo in &from_account_utxos {
        tx_builder
            .spending_plutus_script_v3()
            .tx_in(
                &from_utxo.input.tx_hash,
                from_utxo.input.output_index,
                &from_utxo.output.amount,
                &from_utxo.output.address,
            )
            .tx_in_inline_datum_present()
            .tx_in_redeemer_value(&WRedeemer {
                data: account_balance_spend.redeemer(HydraAccountRedeemer::HydraAccountOperate),
                ex_units: Budget::default(),
            })
            .spending_tx_in_reference(
                collateral.input.tx_hash.as_str(),
                l2_ref_scripts_index::hydra_account_balance::SPEND,
                &account_balance_spend.hash,
                account_balance_spend.cbor.len() / 2,
            )
            .input_for_evaluation(&from_utxo);
    }

    for asset in from_updated_balance_l1 {
        tx_builder
            .tx_out(
                account_balance_address,
                &to_hydra_token(std::slice::from_ref(&asset)),
            )
            .tx_out_inline_datum_value(&WData::JSON(from_account.to_json_string()));

        from_unit_tx_index_map.insert(
            current_index.to_string(),
            AssetList {
                assets: to_proto_amount(std::slice::from_ref(&asset)),
            },
        );
        current_index += 1;
    }

    for asset in to_updated_balance_l2 {
        tx_builder
            .tx_out(account_balance_address, std::slice::from_ref(&asset))
            .tx_out_inline_datum_value(&WData::JSON(to_account.to_json_string()));

        let l1_assets = to_l1_assets(std::slice::from_ref(&asset), all_hydra_to_l1_token_map())
            .map_err(WError::from_err("to_l1_assets"))?;

        to_unit_tx_index_map.insert(
            current_index.to_string(),
            AssetList {
                assets: to_proto_amount(&l1_assets),
            },
        );
        current_index += 1;
    }

    tx_builder
        .mint_plutus_script_v3()
        .mint(-1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&WRedeemer {
            data: user_intent_mint.redeemer(HydraUserIntentRedeemer::BurnIntent),
            ex_units: Budget::default(),
        })
        .mint_tx_in_reference(
            &collateral.input.tx_hash,
            l2_ref_scripts_index::hydra_user_intent::MINT,
            &user_intent_mint.hash,
            user_intent_mint.cbor.len() / 2,
        )
        .input_for_evaluation(&intent_mint_ref_utxo)
        // withdrawal logic
        .withdrawal_plutus_script_v3()
        .withdrawal(&internal_transfer_withdraw.address, 0)
        .withdrawal_redeemer_value(&WRedeemer {
            data: internal_transfer_withdraw.redeemer(HydraAccountOperation::ProcessTransferal),
            ex_units: Budget::default(),
        })
        .withdrawal_tx_in_reference(
            &collateral.input.tx_hash,
            l2_ref_scripts_index::hydra_account_balance::WITHDRAWAL,
            &internal_transfer_withdraw.hash,
            internal_transfer_withdraw.cbor.len() / 2,
        )
        .input_for_evaluation(&account_balance_withdrawal_ref_utxo)
        .tx_in_collateral(
            &collateral.input.tx_hash,
            collateral.input.output_index,
            &collateral.output.amount,
            &collateral.output.address,
        )
        .input_for_evaluation(&collateral)
        .change_address(&request.address)
        .required_signer_hash(&app_owner_vkey)
        .complete(None)
        .await?;

    let tx_hex = tx_builder.tx_hex();
    let tx_hash = calculate_tx_hash(&tx_hex)?;
    let signed_tx = check_signature_sign_tx(app_owner_wallet, &tx_hex)?;

    Ok(ProcessTransferResponse {
        signed_tx,
        tx_hash,
        account_utxo_tx_index_unit_map: if from_unit_tx_index_map.is_empty() {
            None
        } else {
            Some(UnitTxIndexMap {
                unit_tx_index_map: from_unit_tx_index_map,
            })
        },
        receiver_account_utxo_tx_index_unit_map: if to_unit_tx_index_map.is_empty() {
            None
        } else {
            Some(UnitTxIndexMap {
                unit_tx_index_map: to_unit_tx_index_map,
            })
        },
    })
}

#[cfg(test)]
mod active_tests {
    use crate::utils::wallet::get_app_owner_wallet;

    use super::*;
    use dotenv::dotenv;
    use hibiki_proto::services::{AccountInfo, Asset, UTxO, UtxoInput, UtxoOutput};
    use hibiki_proto::services::{BalanceUtxos, ProcessTransferRequest};

    #[test]
    fn test_process_transfer() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(test_process_transfer_case_1());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn test_process_transfer_case_1() {
        dotenv().ok();

        unsafe {
            std::env::set_var(
                "DEX_ORACLE_NFT",
                "9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6d",
            )
        };
        unsafe {
            std::env::set_var(
                "USDM_UNIT",
                "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
            )
        };
        unsafe {
            std::env::set_var(
                "OWNER_VKEY",
                "fa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c",
            )
        };
        unsafe {
            std::env::set_var(
            "APP_OWNER_SEED_PHRASE",
            "trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade,trade",
        );
        }
        unsafe {
            std::env::set_var(
            "FEE_COLLECTOR_SEED_PHRASE",
            "summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer,summer",
        );
        }

        let request = ProcessTransferRequest {
            account: Some(AccountInfo {
                account_id: "08180df3-05ee-4391-8132-4b0775b45f36".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "fdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4".to_string(),
                is_script_master_key: false,
                operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                is_script_operation_key: false,
            }),
            receiver_account: Some(AccountInfo {
                account_id: "45904491-7ccb-444c-bb23-43c1ae02016a".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66".to_string(),
                is_script_master_key: false,
                operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                is_script_operation_key: false,
            }),
            transferral_intent_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "80990368ce151551a67ee1758f7fb9f75f0c69de7ef7c0c73a8ae08c0ff1e0a5".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wqen5pwawre7mkl4d42yr4673fgnc6awu7htu5zhx5dwshcmvju36".to_string(),
                    amount: vec![
                        Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                        Asset { unit: "333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f".to_string(), quantity: "1".to_string() },
                    ],
                    data_hash: "1ff3933b1688ee2040e35a14cad672dfd66b3f099aa42aba3323a47f986afe40".to_string(),
                    plutus_data: "d87a9fd8799fd8799f5008180df305ee439181324b0775b45f36d8799f581cfdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c2cedf51118d0e78d46062fd4be09e625e3b3a0cb78881639b5807a91ffd87b9fd8799fd8799f50459044917ccb444cbb2343c1ae02016ad8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c2cedf51118d0e78d46062fd4be09e625e3b3a0cb78881639b5807a91ffa1581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816a2401a009896805820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e1a00989680ffff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            address: "addr_test1qr77kjlsarq8wy22g4flrcznjh5lkug5mvth7qhhkewgmezwvc8hnnjzy82j5twzf8dfy5gjk04yd09t488ys9605dvq4ymc4x".to_string(),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "e4d920a04b1f6197cc9565c30ce33bd3805f8805ca7b6b1802ef369c229c0dca".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1vra9zdhfa8kteyr3mfe7adkf5nlh8jl5xcg9e7pcp5w9yhq5exvwh".to_string(),
                    amount: vec![Asset { unit: "lovelace".to_string(), quantity: "10000000".to_string() }],
                    data_hash: "".to_string(),
                    plutus_data: "".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "0a730cdc62c8f28d78930a9d9e40991a4fcc5611b1b7b0b85e88c1a502a82d92".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wrcdptezp2cdpn4gm0c72xljvzjgvapfnnvtsv34zuefe9q70mdxj".to_string(),
                    amount: vec![
                        Asset { unit: "lovelace".to_string(), quantity: "6000000".to_string() },
                        Asset { unit: "9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6d".to_string(), quantity: "1".to_string() },
                    ],
                    data_hash: "82d061db86c6197e533203d4c142bbacdb4fc5b0ea2d32ad2762a166dd1d4cad".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525cd8799fd8799f50459044917ccb444cbb2343c1ae02016ad8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c2cedf51118d0e78d46062fd4be09e625e3b3a0cb78881639b5807a91ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581c0a4af222798c805464ec76ec9f837a7829a6b07b54953eb8c38db405581c2cedf51118d0e78d46062fd4be09e625e3b3a0cb78881639b5807a91581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            account_balance_utxos: Some(BalanceUtxos {
                utxos: vec![
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 0,
                            tx_hash: "64d36e5fc925043549d561051781b81436c5ae553af53762087d9090a8c42c6b".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wq9y4u3z0xxgq4rya3mwe8ur0fuznf4s0d2f204ccwxmgpgw9twn0".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset { unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816".to_string(), quantity: "5000000000".to_string() },
                            ],
                            data_hash: "77a29b08e5da44a516d862a7e38ac8fd5ec4aa8f9801b4f120924f1c44223f81".to_string(),
                            plutus_data: "d8799fd8799f5008180df305ee439181324b0775b45f36d8799f581cfdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c2cedf51118d0e78d46062fd4be09e625e3b3a0cb78881639b5807a91ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 5,
                            tx_hash: "64d36e5fc925043549d561051781b81436c5ae553af53762087d9090a8c42c6b".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wq9y4u3z0xxgq4rya3mwe8ur0fuznf4s0d2f204ccwxmgpgw9twn0".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset { unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(), quantity: "5000000000".to_string() },
                            ],
                            data_hash: "77a29b08e5da44a516d862a7e38ac8fd5ec4aa8f9801b4f120924f1c44223f81".to_string(),
                            plutus_data: "d8799fd8799f5008180df305ee439181324b0775b45f36d8799f581cfdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c2cedf51118d0e78d46062fd4be09e625e3b3a0cb78881639b5807a91ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                ],
                updated_balance_l1: vec![
                    Asset { unit: "lovelace".to_string(), quantity: "4990000000".to_string() },
                    Asset { unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(), quantity: "4990000000".to_string() },
                ],
            }),
        };

        let app_owner_wallet = get_app_owner_wallet();
        let result = handler(request, &app_owner_wallet).await;
        println!("Result: {:?}", result);
        assert!(result.is_ok());
    }
}
