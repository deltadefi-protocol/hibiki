use hibiki_proto::services::{SameAccountTransferalRequest, SameAccountTransferalResponse};
use whisky::{calculate_tx_hash, data::PlutusDataJson, WData, WError, Wallet};

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
            .tx_in_redeemer_value(
                &hydra_account_spend.redeemer(HydraAccountRedeemer::HydraAccountOperate, None),
            )
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
            account_id: "7f572d8d-0442-471d-aeb7-e154000f7069".to_string(),
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
                    tx_hash: "f20cc38e5128a70d3128ed5f8f2b9911013f047b75d41467e9b4663e898a24e2".to_string(),
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
                            output_index: 1,
                            tx_hash: "cf6428b1681d059ce37464cb41ac943263e96e24c800eb101f44700120bbe232".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816".to_string(),
                                    quantity: "400".to_string(),
                                },
                            ],
                            data_hash: "72eb1aa8a4a76ce85606bd127274d715cc422fe8a0eec4fbf2430f5710e51b85".to_string(),
                            plutus_data: "d8799fd8799f507f572d8d0442471daeb7e154000f7069d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 2,
                            tx_hash: "cf6428b1681d059ce37464cb41ac943263e96e24c800eb101f44700120bbe232".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset {
                                    unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(),
                                    quantity: "338".to_string(),
                                },
                            ],
                            data_hash: "72eb1aa8a4a76ce85606bd127274d715cc422fe8a0eec4fbf2430f5710e51b85".to_string(),
                            plutus_data: "d8799fd8799f507f572d8d0442471daeb7e154000f7069d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                ],
                updated_balance_l1: vec![
                    Asset {
                        unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                        quantity: "338".to_string(),
                    },
                    Asset {
                        unit: "lovelace".to_string(),
                        quantity: "400".to_string(),
                    },
                ],
            }),
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "56836e1e8693a04df6a36cae178d01e9e0ea85b84cdcd8a815014a6e04f18b90".to_string(),
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
                    data_hash: "883f9483d3a15425311c9d35ec55f33672f86a5f4529f9b17155405a2c11674f".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f507f572d8d0442471daeb7e154000f7069d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581cbef30f7146f3370715356f4f88c64928d62708afec6796ddf1070b88581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
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
        let result = handler(request, &app_owner_wallet, &config, &scripts)
            .await
            .unwrap();
        println!("result.signed_tx: {:?}", result.signed_tx);
        assert_eq!(
            result.tx_hash,
            "670e1d81f31449fef50e8877fb6ef8345e7cb160b5bb948d32e8a0f6ce32be64"
        );
    }
}
