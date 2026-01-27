use hibiki_proto::services::{IntentTxResponse, InternalTransferRequest};
use whisky::{calculate_tx_hash, data::PlutusDataJson, Asset, WData, WError};

use crate::{
    config::AppConfig,
    scripts::{
        HydraAccountIntent, HydraUserIntentDatum, HydraUserIntentRedeemer, ScriptCache, UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{assets_to_mvalue, from_proto_amount, from_proto_utxo},
        token::to_hydra_token,
    },
};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub async fn handler(
    request: InternalTransferRequest,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<IntentTxResponse, WError> {
    let app_owner_vkey = &config.app_owner_vkey;
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let collateral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let empty_utxo = from_proto_utxo(request.empty_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(request.dex_order_book_utxo.as_ref().unwrap());
    let account = request.account.unwrap();

    let mut tx_builder = get_hydra_tx_builder();
    let user_intent_mint = &scripts.user_intent_mint;
    let user_intent_spend = &scripts.user_intent_spend;

    let from_account = UserAccount::from_proto_trade_account(&account, account_ops_script_hash);
    let to_account = UserAccount::from_proto_trade_account(
        &request.receiver_account.unwrap(),
        account_ops_script_hash,
    );
    let transfer_amount_l2 =
        assets_to_mvalue(&to_hydra_token(&from_proto_amount(&request.to_transfer)));

    // Create transfer intent
    let hydra_account_intent =
        HydraAccountIntent::TransferIntent(Box::new((to_account.clone(), transfer_amount_l2)));
    let intent = Box::new((from_account, hydra_account_intent));

    tx_builder
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        .mint_plutus_script_v3()
        .mint(1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&user_intent_mint.redeemer(
            HydraUserIntentRedeemer::MintMasterIntent(intent.clone()),
            None,
        ))
        .mint_tx_in_reference(
            &collateral.input.tx_hash,
            user_intent_mint.ref_output_index,
            &user_intent_mint.hash,
            user_intent_mint.size,
        )
        .input_for_evaluation(&user_intent_mint.ref_utxo(&collateral)?)
        .tx_out(
            &user_intent_spend.address,
            &[Asset::new_from_str(&user_intent_mint.hash, "1")],
        )
        .tx_out_inline_datum_value(&WData::JSON(
            HydraUserIntentDatum::MasterIntent(intent).to_json_string(),
        ))
        .tx_in(
            &empty_utxo.input.tx_hash,
            empty_utxo.input.output_index,
            &empty_utxo.output.amount,
            &empty_utxo.output.address,
        )
        .input_for_evaluation(&empty_utxo)
        .tx_out(&empty_utxo.output.address, &empty_utxo.output.amount)
        .required_signer_hash(&app_owner_vkey)
        .required_signer_hash(&account.master_key)
        .tx_in_collateral(
            &collateral.input.tx_hash,
            collateral.input.output_index,
            &collateral.output.amount,
            &collateral.output.address,
        )
        .input_for_evaluation(&collateral)
        .change_address(&request.address);

    log::debug!("[INTERNAL_TRANSFER] tx_builder.mint_item: {:?}", tx_builder.mint_item);

    tx_builder.complete(None).await?;

    let tx_hex = tx_builder.tx_hex();
    let tx_hash = calculate_tx_hash(&tx_hex)?;

    Ok(IntentTxResponse {
        tx_hex,
        tx_hash,
        tx_index: 0,
        new_empty_utxo_tx_index: 1,
    })
}

#[cfg(test)]
mod tests {
    use hibiki_proto::services::{AccountInfo, Asset, UTxO, UtxoInput, UtxoOutput};

    use super::*;
    use crate::scripts::{ScriptCache, UserAccount};
    use crate::test_fixtures::{build_collateral_utxo, build_dex_order_book_utxo, L2ScriptConfig};
    use crate::test_utils::init_test_env;

    #[test]
    fn test_internal_transfer() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(test_internal_transfer_case_1());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn test_internal_transfer_case_1() {
        init_test_env();

        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();

        // Create a fee account for the dex_order_book_utxo datum
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

        // Build dex_order_book_utxo dynamically with current script hashes
        let dex_order_book_utxo = build_dex_order_book_utxo(
            &scripts,
            &l2_config,
            "92618472886c4d9c90b39d700371a97aa1164ac8103609577035e96f7791998c",
            0,
            &fee_account,
        )
        .expect("Failed to build dex_order_book_utxo");

        let request = InternalTransferRequest {
            account: Some(AccountInfo {
                account_id: "86acc254-1528-4007-a75f-32075fc1b609".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
                is_script_master_key: false,
                operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                is_script_operation_key: false,
            }),
            receiver_account: Some(AccountInfo {
                account_id: "2c4d8d95-3144-4be0-b9f7-7d32a2008798".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "762bb328796001c99bd22819544a11bf493b62224af52b3dff9b0add".to_string(),
                is_script_master_key: false,
                operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                is_script_operation_key: false,
            }),
            to_transfer: vec![
                Asset {
                    unit: "lovelace".to_string(),
                    quantity: "23506875".to_string(),
                },
                Asset {
                    unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                    quantity: "47013750000".to_string(),
                },
                Asset {
                    unit: "3363b99384d6ee4c4b009068af396c8fdf92dafd111e58a857af04294e49474854".to_string(),
                    quantity: "47013750000".to_string(),
                },
                Asset {
                    unit: "378f9732c755ed6f4fc8d406f1461d0cca95d7d2e69416784684df39534e454b".to_string(),
                    quantity: "47013750000".to_string(),
                },
                Asset {
                    unit: "82e46eb16633bf8bfa820c83ffeb63192c6e21757d2bf91290b2f41d494147".to_string(),
                    quantity: "47013750000".to_string(),
                },
                Asset {
                    unit: "a2818ba06a88bb6c08d10f4f9b897c09768f28d274093628ad7086fc484f534b59".to_string(),
                    quantity: "47013750000".to_string(),
                },
            ],
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            collateral_utxo: Some(build_collateral_utxo(
                "91fddaab7baf528c4c67c67da8bf20e1de482037b78fb836963de24fdee3d45f",
                0,
                "10000000",
            )),
            empty_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 1,
                    tx_hash: "85c29440753e4f4d3c5c634b6c7b7eb35af5e3a5f9465fdbb4b80284d499e17e".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1qra9zdhfa8kteyr3mfe7adkf5nlh8jl5xcg9e7pcp5w9yhyf5tek6vpnha97yd5yw9pezm3wyd77fyrfs3ynftyg7njs5cfz2x".to_string(),
                    amount: vec![Asset {
                        unit: "lovelace".to_string(),
                        quantity: "0".to_string(),
                    }],
                    data_hash: "".to_string(),
                    plutus_data: "".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            dex_order_book_utxo: Some(dex_order_book_utxo),
        };

        let config = AppConfig::new();
        let result = handler(request, &config, &scripts).await;
        println!("Result: {:?}", result);
        assert!(result.is_ok());
    }
}
