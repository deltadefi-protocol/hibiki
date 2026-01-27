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

#[cfg(test)]
mod active_tests {
    use crate::scripts::UserAccount;
    use crate::test_fixtures::{
        build_account_balance_utxo, build_collateral_utxo, build_dex_order_book_utxo,
        build_transfer_intent_utxo, L2ScriptConfig,
    };
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;

    use super::*;
    use hibiki_proto::services::{AccountInfo, Asset};
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

        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();

        // Create sender account
        let from_account_info = AccountInfo {
            account_id: "86acc254-1528-4007-a75f-32075fc1b609".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };
        let from_account = UserAccount::from_proto_trade_account(
            &from_account_info,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Create receiver account
        let to_account_info = AccountInfo {
            account_id: "bb0f539c-996c-4725-8f8b-918186ae2b5b".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "50cd5eafde7b00ea5d7d592d8a904363fa9229a8fc32e692f4e0a748".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };
        let to_account = UserAccount::from_proto_trade_account(
            &to_account_info,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Create fee account for dex_order_book_utxo
        let fee_account_info = AccountInfo {
            account_id: "a810ec7a-7069-4157-aead-48ecf4d693c8".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };
        let fee_account = UserAccount::from_proto_trade_account(
            &fee_account_info,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Transfer amounts (L1 tokens that will be converted to L2 in the intent)
        let transfer_amounts = vec![
            Asset { unit: "lovelace".to_string(), quantity: "23506875".to_string() },
            Asset { unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(), quantity: "47013750000".to_string() },
            Asset { unit: "3363b99384d6ee4c4b009068af396c8fdf92dafd111e58a857af04294e49474854".to_string(), quantity: "47013750000".to_string() },
            Asset { unit: "378f9732c755ed6f4fc8d406f1461d0cca95d7d2e69416784684df39534e454b".to_string(), quantity: "47013750000".to_string() },
            Asset { unit: "82e46eb16633bf8bfa820c83ffeb63192c6e21757d2bf91290b2f41d494147".to_string(), quantity: "47013750000".to_string() },
            Asset { unit: "a2818ba06a88bb6c08d10f4f9b897c09768f28d274093628ad7086fc484f534b59".to_string(), quantity: "47013750000".to_string() },
        ];

        // Build dex_order_book_utxo dynamically
        let dex_order_book_utxo = build_dex_order_book_utxo(
            &scripts,
            &l2_config,
            "92618472886c4d9c90b39d700371a97aa1164ac8103609577035e96f7791998c",
            0,
            &fee_account,
        )
        .expect("Failed to build dex_order_book_utxo");

        // Build transfer intent UTxO dynamically
        let transferral_intent_utxo = build_transfer_intent_utxo(
            &scripts,
            "cfb7bf9778544b2e06bc3d4d3def2fd17f961094e0c7c59e291029e8bc90e6da",
            0,
            &from_account,
            &to_account,
            &transfer_amounts,
        )
        .expect("Failed to build transferral_intent_utxo");

        // Build account balance UTxOs dynamically using hydra tokens (L2)
        // Each UTxO has one L2 token (hydra_token_mint hash + L2 token name)
        let hydra_token_hash = &scripts.hydra_token_mint.hash;

        // L2 token names are hashes of L1 units
        let l2_tokens = vec![
            // HOSKY - hash of a2818ba06a88bb6c08d10f4f9b897c09768f28d274093628ad7086fc484f534b59
            ("51dc102c9a98480d045c5efe923d55ea4f3202eb0de800478993aaf678cafe78", "9308722500000"),
            // NIGHT - hash of 3363b99384d6ee4c4b009068af396c8fdf92dafd111e58a857af04294e49474854
            ("5eed9aa5425511535d3bfa2b16f77f27848e438a0e13fe233c1adc25d242a88d", "9308722500000"),
            // lovelace (empty asset name)
            ("", "4654361250"),
            // USDM - hash of c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d
            ("ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e", "9308722500000"),
            // SNEK - hash of 378f9732c755ed6f4fc8d406f1461d0cca95d7d2e69416784684df39534e454b
            ("d2eaf64d54d94a87b00abd2f745c7181d909b37c9c97fd0287ad4b6236abb35e", "9308722500000"),
            // IAG - hash of 82e46eb16633bf8bfa820c83ffeb63192c6e21757d2bf91290b2f41d494147
            ("7e8f6cc9a5dbf788ea70234aac56977b0fc8a2fe3cbfe565e16dd63ba00ea54a", "9308722500000"),
        ];

        let mut account_balance_utxos = Vec::new();
        for (i, (l2_name, quantity)) in l2_tokens.iter().enumerate() {
            let l2_unit = format!("{}{}", hydra_token_hash, l2_name);
            let balance_asset = Asset {
                unit: l2_unit,
                quantity: quantity.to_string(),
            };
            let utxo = build_account_balance_utxo(
                &scripts,
                "9d8f634f368f158569b11101041bab5ba66772bde329c9e7a1e1700e28f195ed",
                i as u32,
                &from_account,
                &balance_asset,
            )
            .expect("Failed to build account_balance_utxo");
            account_balance_utxos.push(utxo);
        }

        let request = ProcessTransferRequest {
            account: Some(from_account_info),
            receiver_account: Some(to_account_info),
            transferral_intent_utxo: Some(transferral_intent_utxo),
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            collateral_utxo: Some(build_collateral_utxo(
                "91fddaab7baf528c4c67c67da8bf20e1de482037b78fb836963de24fdee3d45f",
                0,
                "10000000",
            )),
            dex_order_book_utxo: Some(dex_order_book_utxo),
            account_balance_utxos: Some(BalanceUtxos {
                utxos: account_balance_utxos,
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
        let result = handler(request, &app_owner_wallet, &config, &scripts).await;
        println!("Result: {:?}", result);
        assert!(result.is_ok());
    }
}
