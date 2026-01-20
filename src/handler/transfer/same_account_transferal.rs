use hibiki_proto::services::{SameAccountTransferalRequest, SameAccountTransferalResponse};
use whisky::{
    calculate_tx_hash,
    data::{PlutusData, PlutusDataJson},
    WData, WError, Wallet,
};

use crate::{
    config::AppConfig,
    handler::sign_transaction::check_signature_sign_tx,
    scripts::{HydraAccountOperation, HydraAccountRedeemer, ScriptCache, UserAccount},
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{from_proto_balance_utxos, from_proto_utxo, TxIndexAssetsMap},
        token::to_hydra_token,
    },
};

pub async fn handler(
    request: SameAccountTransferalRequest,
    app_owner_wallet: &Wallet,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<SameAccountTransferalResponse, WError> {
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let app_owner_vkey = &config.app_owner_vkey;
    let collateral = from_proto_utxo(&request.collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(&request.dex_order_book_utxo.as_ref().unwrap());
    let account = UserAccount::from_proto_trade_account(
        &request.account.as_ref().unwrap(),
        &account_ops_script_hash,
    );

    // Parse balance UTXOs
    let (updated_balance_l1, balance_utxos) =
        from_proto_balance_utxos(&request.account_balance_utxos.as_ref().unwrap());

    // Get script address for outputs
    let account_balance_address = &balance_utxos[0].output.address;

    // Use cached script info
    let hydra_account_spend = &scripts.hydra_account_spend;
    let hydra_account_withdrawal = &scripts.hydra_account_withdrawal;

    let mut unit_tx_index_map = TxIndexAssetsMap::new(updated_balance_l1.len());

    // Build script reference UTxOs using cached script info
    let mut tx_builder = get_hydra_tx_builder();

    // Reference oracle UTxO
    tx_builder
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        .input_for_evaluation(&hydra_account_spend.ref_utxo(&collateral)?)
        .input_for_evaluation(&hydra_account_withdrawal.ref_utxo(&collateral)?);

    // Spend all balance UTxOs using HydraAccountOperate (Constr1)
    for utxo in &balance_utxos {
        tx_builder
            .spending_plutus_script_v3()
            .tx_in(
                &utxo.input.tx_hash,
                utxo.input.output_index,
                &utxo.output.amount,
                &utxo.output.address,
            )
            .tx_in_inline_datum_present()
            .tx_in_redeemer_value(&hydra_account_spend.redeemer(
                HydraAccountRedeemer::<PlutusData>::HydraAccountOperate,
                None,
            ))
            .spending_tx_in_reference(
                &collateral.input.tx_hash,
                scripts.hydra_account_spend.ref_output_index,
                &hydra_account_spend.hash,
                hydra_account_spend.size,
            )
            .input_for_evaluation(&utxo);
    }

    // Create outputs for each asset in updated balance
    for asset in updated_balance_l1 {
        let hydra_asset = to_hydra_token(std::slice::from_ref(&asset));
        tx_builder
            .tx_out(account_balance_address, &hydra_asset)
            .tx_out_inline_datum_value(&WData::JSON(account.to_json_string()));
        unit_tx_index_map.insert(&[asset]);
    }

    tx_builder
        .withdrawal_plutus_script_v3()
        .withdrawal(&hydra_account_withdrawal.address, 0)
        .withdrawal_redeemer_value(&hydra_account_withdrawal.redeemer(
            HydraAccountOperation::ProcessSameAccountTransferal(account),
            None,
        ))
        .withdrawal_tx_in_reference(
            &collateral.input.tx_hash,
            scripts.hydra_account_withdrawal.ref_output_index,
            &hydra_account_withdrawal.hash,
            hydra_account_withdrawal.size,
        )
        .input_for_evaluation(&hydra_account_withdrawal.ref_utxo(&collateral)?)
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

    Ok(SameAccountTransferalResponse {
        signed_tx,
        tx_hash,
        account_utxo_tx_index_unit_map: unit_tx_index_map.to_proto(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;
    use hibiki_proto::services::{AccountInfo, Asset, BalanceUtxos, UTxO, UtxoInput, UtxoOutput};

    #[test]
    fn test_same_account_transferal() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(test_same_account_transferal_case_1());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn test_same_account_transferal_case_1() {
        init_test_env();

        let account = AccountInfo {
            account_id: "326b8949-4e38-47c2-8887-23e7c5d3d654".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        let request = SameAccountTransferalRequest {
            address: "addr_test1qqzgg5pcaeyea69uptl9da5g7fajm4m0yvxndx9f4lxpkehqgezy0s04rtdwlc0tlvxafpdrfxnsg7ww68ge3j7l0lnszsw2wt".to_string(),
            account: Some(account),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "91fddaab7baf528c4c67c67da8bf20e1de482037b78fb836963de24fdee3d45f".to_string(),
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
            account_balance_utxos: Some(BalanceUtxos {
                utxos: vec![
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 2,
                            tx_hash: "eb22bac72f0c70b20a3f5f4ba958c46f7af00c1a886b94e73cae8d62193bb979".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "1321".to_string(),
                                },
                            ],
                            data_hash: "5ea685b640054e1dc6db005d22b05000410a549a0ca850dd250a95c441c0fd8e".to_string(),
                            plutus_data: "d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 4,
                            tx_hash: "4e9644edf42b5c57e15ad1e4fb8e0e7b834223b648d6cc267c2953e680c1f24b".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "1745".to_string(),
                                },
                            ],
                            data_hash: "5ea685b640054e1dc6db005d22b05000410a549a0ca850dd250a95c441c0fd8e".to_string(),
                            plutus_data: "d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 4,
                            tx_hash: "5fe49cf6ebccb104c4072bc83bcfcc4ef53a389bb99285eb1ac242d76039570b".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "1211".to_string(),
                                },
                            ],
                            data_hash: "5ea685b640054e1dc6db005d22b05000410a549a0ca850dd250a95c441c0fd8e".to_string(),
                            plutus_data: "d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 4,
                            tx_hash: "45a7cae12dfc8c5c183d882de265adab5f47436c89b952e9a407fb7a9a81dae8".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "1235".to_string(),
                                },
                            ],
                            data_hash: "5ea685b640054e1dc6db005d22b05000410a549a0ca850dd250a95c441c0fd8e".to_string(),
                            plutus_data: "d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 4,
                            tx_hash: "c08f45e13eebdb9657612066c69181ee41975b9f9ed4a39b3cb00274f13d6e43".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "1024".to_string(),
                                },
                            ],
                            data_hash: "5ea685b640054e1dc6db005d22b05000410a549a0ca850dd250a95c441c0fd8e".to_string(),
                            plutus_data: "d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 4,
                            tx_hash: "b1489b2877a4305c91deb3bf56ea871db390571622cbf87199cc36f4bef4b786".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "1606".to_string(),
                                },
                            ],
                            data_hash: "5ea685b640054e1dc6db005d22b05000410a549a0ca850dd250a95c441c0fd8e".to_string(),
                            plutus_data: "d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 4,
                            tx_hash: "e4341bf07f63dbe21e236655926063e1074da3987a986ac249f27ccee50e1875".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "683".to_string(),
                                },
                            ],
                            data_hash: "5ea685b640054e1dc6db005d22b05000410a549a0ca850dd250a95c441c0fd8e".to_string(),
                            plutus_data: "d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 4,
                            tx_hash: "abbc606ff1d23538b5740883ea415340ce4fc4ab37fbca9593dafbea26c889d8".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "1838".to_string(),
                                },
                            ],
                            data_hash: "5ea685b640054e1dc6db005d22b05000410a549a0ca850dd250a95c441c0fd8e".to_string(),
                            plutus_data: "d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                ],
                updated_balance_l1: vec![
                    Asset {
                        unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                        quantity: "10663".to_string(),
                    },
                ],
            }),
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "92618472886c4d9c90b39d700371a97aa1164ac8103609577035e96f7791998c".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wrcdptezp2cdpn4gm0c72xljvzjgvapfnnvtsv34zuefe9q70mdxj".to_string(),
                    amount: vec![
                        Asset { unit: "lovelace".to_string(), quantity: "6000000".to_string() },
                        Asset {
                            unit: "9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6d".to_string(),
                            quantity: "1".to_string(),
                        },
                    ],
                    data_hash: "93fc6a09dc385a32ab604ef5bdcfec071121e05f2281fe207168fa576714a371".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581cbef30f7146f3370715356f4f88c64928d62708afec6796ddf1070b88581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
        };

        println!("=== Input Data ===");
        println!("Address: {}", request.address);
        println!(
            "Account ID: {}",
            request.account.as_ref().unwrap().account_id
        );
        println!(
            "Balance UTxOs count: {}",
            request.account_balance_utxos.as_ref().unwrap().utxos.len()
        );

        let config = AppConfig::new();
        let scripts = ScriptCache::new();
        let app_owner_wallet = get_app_owner_wallet();
        let result = handler(request, &app_owner_wallet, &config, &scripts).await;

        match result {
            Ok(response) => {
                println!("result.tx_hash: {:?}", response.tx_hash);
                println!("result.signed_tx: {:?}", response.signed_tx);
                println!(
                    "result.account_utxo_tx_index_unit_map: {:?}",
                    response.account_utxo_tx_index_unit_map
                );
            }
            Err(e) => {
                panic!("Test failed with error: {:?}", e);
            }
        }
    }
}
