use hibiki_proto::services::{ProcessTransferRequest, ProcessTransferResponse};
use whisky::{
    calculate_tx_hash,
    data::{ByteString, PlutusData, PlutusDataJson},
    PlutusDataCbor, WData, WError, Wallet,
};

use crate::{
    config::{constant::all_hydra_to_l1_token_map, AppConfig},
    handler::sign_transaction::check_signature_sign_tx,
    scripts::{
        HydraAccountIntent, HydraAccountOperation, HydraAccountRedeemer, HydraUserIntentDatum,
        HydraUserIntentRedeemer, ScriptCache, UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{from_proto_balance_utxos, from_proto_utxo, TxIndexAssetsMap},
        token::{to_hydra_token, to_l1_assets},
    },
};

pub async fn handler(
    request: ProcessTransferRequest,
    app_owner_wallet: &Wallet,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<ProcessTransferResponse, WError> {
    let app_owner_vkey = &config.app_owner_vkey;
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let collateral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(request.dex_order_book_utxo.as_ref().unwrap());
    let intent_utxo = from_proto_utxo(request.transferral_intent_utxo.as_ref().unwrap());
    let from_account = UserAccount::from_proto_trade_account(
        request.account.as_ref().unwrap(),
        account_ops_script_hash,
    );
    let to_account = UserAccount::from_proto_trade_account(
        request.receiver_account.as_ref().unwrap(),
        account_ops_script_hash,
    );

    // Parse sender's balance UTXOs
    let (from_updated_balance_l1, from_account_utxos) =
        from_proto_balance_utxos(request.account_balance_utxos.as_ref().unwrap());

    let intent_datum =
        HydraUserIntentDatum::<HydraAccountIntent>::from_cbor(
            &intent_utxo.output.plutus_data.as_ref().ok_or_else(|| {
                WError::new("process_transfer", "Missing plutus_data in intent_utxo")
            })?,
        )?;
    let to_updated_balance_l2 = intent_datum.get_transfer_amount()?;

    // For outputs, we need one UTXO address - use the first sender's UTXO address as template
    let account_balance_address = &from_account_utxos[0].output.address;

    let user_intent_mint = &scripts.user_intent_mint;
    let user_intent_spend = &scripts.user_intent_spend;
    let account_balance_spend = &scripts.hydra_account_spend;
    let internal_transfer_withdraw = &scripts.hydra_account_withdrawal;

    let mut from_unit_tx_index_map = TxIndexAssetsMap::new(from_updated_balance_l1.len());
    let mut to_unit_tx_index_map = TxIndexAssetsMap::new(to_updated_balance_l2.len());

    // Build script reference UTxOs using cached script info
    let mint_ref_utxo = user_intent_mint.ref_utxo(&collateral)?;
    let spend_ref_utxo = user_intent_spend.ref_utxo(&collateral)?;
    let balance_spend_ref_utxo = account_balance_spend.ref_utxo(&collateral)?;
    let balance_withdrawal_ref_utxo = internal_transfer_withdraw.ref_utxo(&collateral)?;

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
        .tx_in_redeemer_value(&user_intent_spend.redeemer(ByteString::new(""), None))
        .spending_tx_in_reference(
            collateral.input.tx_hash.as_str(),
            user_intent_spend.ref_output_index,
            &user_intent_spend.hash,
            user_intent_spend.size,
        )
        .input_for_evaluation(&spend_ref_utxo)
        .input_for_evaluation(&intent_utxo)
        .input_for_evaluation(&balance_spend_ref_utxo);

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
            .tx_in_redeemer_value(&account_balance_spend.redeemer(
                HydraAccountRedeemer::<PlutusData>::HydraAccountOperate,
                None,
            ))
            .spending_tx_in_reference(
                collateral.input.tx_hash.as_str(),
                account_balance_spend.ref_output_index,
                &account_balance_spend.hash,
                account_balance_spend.size,
            )
            .input_for_evaluation(&from_utxo);
    }

    let mut current_index = 0u32;
    for asset in from_updated_balance_l1 {
        tx_builder
            .tx_out(
                account_balance_address,
                &to_hydra_token(std::slice::from_ref(&asset)),
            )
            .tx_out_inline_datum_value(&WData::JSON(from_account.to_json_string()));
        from_unit_tx_index_map.insert(&[asset]);
        current_index += 1;
    }

    to_unit_tx_index_map.set_index(current_index);
    for asset in to_updated_balance_l2 {
        tx_builder
            .tx_out(account_balance_address, std::slice::from_ref(&asset))
            .tx_out_inline_datum_value(&WData::JSON(to_account.to_json_string()));
        let l1_assets = to_l1_assets(std::slice::from_ref(&asset), all_hydra_to_l1_token_map())
            .map_err(WError::from_err("to_l1_assets"))?;
        to_unit_tx_index_map.insert(&l1_assets);
    }

    tx_builder
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
        .input_for_evaluation(&mint_ref_utxo)
        // withdrawal logic
        .withdrawal_plutus_script_v3()
        .withdrawal(&internal_transfer_withdraw.address, 0)
        .withdrawal_redeemer_value(
            &internal_transfer_withdraw.redeemer(HydraAccountOperation::ProcessTransferal, None),
        )
        .withdrawal_tx_in_reference(
            &collateral.input.tx_hash,
            internal_transfer_withdraw.ref_output_index,
            &internal_transfer_withdraw.hash,
            internal_transfer_withdraw.size,
        )
        .input_for_evaluation(&balance_withdrawal_ref_utxo)
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
        account_utxo_tx_index_unit_map: from_unit_tx_index_map.to_proto(),
        receiver_account_utxo_tx_index_unit_map: to_unit_tx_index_map.to_proto(),
    })
}

// todo: make tests work with new script
#[cfg(test)]
mod active_tests {
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;

    use super::*;
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
        init_test_env();

        let request = ProcessTransferRequest {
            account: Some(AccountInfo {
                account_id: "86acc254-1528-4007-a75f-32075fc1b609".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
                is_script_master_key: false,
                operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                is_script_operation_key: false,
            }),
            receiver_account: Some(AccountInfo {
                account_id: "bb0f539c-996c-4725-8f8b-918186ae2b5b".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "50cd5eafde7b00ea5d7d592d8a904363fa9229a8fc32e692f4e0a748".to_string(),
                is_script_master_key: false,
                operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                is_script_operation_key: false,
            }),
            transferral_intent_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "cfb7bf9778544b2e06bc3d4d3def2fd17f961094e0c7c59e291029e8bc90e6da".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wqen5pwawre7mkl4d42yr4673fgnc6awu7htu5zhx5dwshcmvju36".to_string(),
                    amount: vec![
                        Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                        Asset { unit: "333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f".to_string(), quantity: "1".to_string() },
                    ],
                    data_hash: "df5cf6100291adb5730610a088be429703cac540e4e848516b276c36bbe03c78".to_string(),
                    plutus_data: "d87a9fd8799fd8799f5086acc25415284007a75f32075fc1b609d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffd87b9fd8799fd8799f50bb0f539c996c47258f8b918186ae2b5bd8799f581c50cd5eafde7b00ea5d7d592d8a904363fa9229a8fc32e692f4e0a748ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ffa1581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816a6401a0166afbb582051dc102c9a98480d045c5efe923d55ea4f3202eb0de800478993aaf678cafe781b0000000af23ce4f058205eed9aa5425511535d3bfa2b16f77f27848e438a0e13fe233c1adc25d242a88d1b0000000af23ce4f058207e8f6cc9a5dbf788ea70234aac56977b0fc8a2fe3cbfe565e16dd63ba00ea54a1b0000000af23ce4f05820ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e1b0000000af23ce4f05820d2eaf64d54d94a87b00abd2f745c7181d909b37c9c97fd0287ad4b6236abb35e1b0000000af23ce4f0ffff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "91fddaab7baf528c4c67c67da8bf20e1de482037b78fb836963de24fdee3d45f".to_string(),
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
                    tx_hash: "92618472886c4d9c90b39d700371a97aa1164ac8103609577035e96f7791998c".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wrcdptezp2cdpn4gm0c72xljvzjgvapfnnvtsv34zuefe9q70mdxj".to_string(),
                    amount: vec![
                        Asset { unit: "lovelace".to_string(), quantity: "6000000".to_string() },
                        Asset { unit: "9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6d".to_string(), quantity: "1".to_string() },
                    ],
                    data_hash: "93fc6a09dc385a32ab604ef5bdcfec071121e05f2281fe207168fa576714a371".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f50326b89494e3847c2888723e7c5d3d654d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581cbef30f7146f3370715356f4f88c64928d62708afec6796ddf1070b88581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            account_balance_utxos: Some(BalanceUtxos {
                utxos: vec![
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 1,
                            tx_hash: "9d8f634f368f158569b11101041bab5ba66772bde329c9e7a1e1700e28f195ed".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset { unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc81651dc102c9a98480d045c5efe923d55ea4f3202eb0de800478993aaf678cafe78".to_string(), quantity: "9308722500000".to_string() },
                            ],
                            data_hash: "3b05c45fcd52c197188d607b878cfa62138795ab7d12829b84244e38ac1e687e".to_string(),
                            plutus_data: "d8799fd8799f5086acc25415284007a75f32075fc1b609d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 2,
                            tx_hash: "9d8f634f368f158569b11101041bab5ba66772bde329c9e7a1e1700e28f195ed".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset { unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc8165eed9aa5425511535d3bfa2b16f77f27848e438a0e13fe233c1adc25d242a88d".to_string(), quantity: "9308722500000".to_string() },
                            ],
                            data_hash: "3b05c45fcd52c197188d607b878cfa62138795ab7d12829b84244e38ac1e687e".to_string(),
                            plutus_data: "d8799fd8799f5086acc25415284007a75f32075fc1b609d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 3,
                            tx_hash: "9d8f634f368f158569b11101041bab5ba66772bde329c9e7a1e1700e28f195ed".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset { unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816".to_string(), quantity: "4654361250".to_string() },
                            ],
                            data_hash: "3b05c45fcd52c197188d607b878cfa62138795ab7d12829b84244e38ac1e687e".to_string(),
                            plutus_data: "d8799fd8799f5086acc25415284007a75f32075fc1b609d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 4,
                            tx_hash: "9d8f634f368f158569b11101041bab5ba66772bde329c9e7a1e1700e28f195ed".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset { unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e".to_string(), quantity: "9308722500000".to_string() },
                            ],
                            data_hash: "3b05c45fcd52c197188d607b878cfa62138795ab7d12829b84244e38ac1e687e".to_string(),
                            plutus_data: "d8799fd8799f5086acc25415284007a75f32075fc1b609d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 5,
                            tx_hash: "9d8f634f368f158569b11101041bab5ba66772bde329c9e7a1e1700e28f195ed".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset { unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816d2eaf64d54d94a87b00abd2f745c7181d909b37c9c97fd0287ad4b6236abb35e".to_string(), quantity: "9308722500000".to_string() },
                            ],
                            data_hash: "3b05c45fcd52c197188d607b878cfa62138795ab7d12829b84244e38ac1e687e".to_string(),
                            plutus_data: "d8799fd8799f5086acc25415284007a75f32075fc1b609d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                    UTxO {
                        input: Some(UtxoInput {
                            output_index: 0,
                            tx_hash: "9d8f634f368f158569b11101041bab5ba66772bde329c9e7a1e1700e28f195ed".to_string(),
                        }),
                        output: Some(UtxoOutput {
                            address: "addr_test1wzl0xrm3gmenwpc4x4h5lzxxfy5dvfcg4lkx09ka7yrshzqpkt4dh".to_string(),
                            amount: vec![
                                Asset { unit: "lovelace".to_string(), quantity: "0".to_string() },
                                Asset { unit: "b28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc8167e8f6cc9a5dbf788ea70234aac56977b0fc8a2fe3cbfe565e16dd63ba00ea54a".to_string(), quantity: "9308722500000".to_string() },
                            ],
                            data_hash: "3b05c45fcd52c197188d607b878cfa62138795ab7d12829b84244e38ac1e687e".to_string(),
                            plutus_data: "d8799fd8799f5086acc25415284007a75f32075fc1b609d8799f581c4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79bffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff".to_string(),
                            script_ref: "".to_string(),
                            script_hash: "".to_string(),
                        }),
                    },
                ],
                updated_balance_l1: vec![
                    Asset { unit: "a2818ba06a88bb6c08d10f4f9b897c09768f28d274093628ad7086fc484f534b59".to_string(), quantity: "9261708750000".to_string() },
                    Asset { unit: "lovelace".to_string(), quantity: "4630854375".to_string() },
                    Asset { unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(), quantity: "9261708750000".to_string() },
                    Asset { unit: "3363b99384d6ee4c4b009068af396c8fdf92dafd111e58a857af04294e49474854".to_string(), quantity: "9261708750000".to_string() },
                    Asset { unit: "378f9732c755ed6f4fc8d406f1461d0cca95d7d2e69416784684df39534e454b".to_string(), quantity: "9261708750000".to_string() },
                    Asset { unit: "82e46eb16633bf8bfa820c83ffeb63192c6e21757d2bf91290b2f41d494147".to_string(), quantity: "9261708750000".to_string() },
                ],
            }),
        };

        let app_owner_wallet = get_app_owner_wallet();
        let config = AppConfig::new();
        let scripts: ScriptCache = ScriptCache::new();
        let result = handler(request, &app_owner_wallet, &config, &scripts).await;
        println!("Result: {:?}", result);
        assert!(result.is_ok());
    }
}
