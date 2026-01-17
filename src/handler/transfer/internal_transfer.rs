use hibiki_proto::services::{IntentTxResponse, InternalTransferRequest};
use whisky::{calculate_tx_hash, data::PlutusDataJson, Asset, Budget, WData, WError, WRedeemer};

use crate::{
    config::AppConfig,
    scripts::{
        l2_ref_scripts_index, HydraAccountIntent, HydraUserIntentDatum, HydraUserIntentRedeemer,
        ScriptCache, UserAccount,
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
    let redeemer_json = HydraUserIntentRedeemer::MintMasterIntent(Box::new((
        from_account.clone(),
        hydra_account_intent.clone(),
    )));
    let datum_json =
        HydraUserIntentDatum::MasterIntent(Box::new((from_account, hydra_account_intent)));

    tx_builder
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        .mint_plutus_script_v3()
        .mint(1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&WRedeemer {
            data: WData::JSON(redeemer_json.to_json_string()),
            ex_units: Budget::default(),
        })
        .mint_tx_in_reference(
            &collateral.input.tx_hash,
            l2_ref_scripts_index::hydra_user_intent::MINT,
            &user_intent_mint.hash,
            user_intent_mint.size,
        )
        .input_for_evaluation(&user_intent_mint.ref_utxo(&collateral)?)
        .tx_out(
            &user_intent_spend.address,
            &[Asset::new_from_str(&user_intent_mint.hash, "1")],
        )
        .tx_out_inline_datum_value(&WData::JSON(datum_json.to_json_string()))
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
        .change_address(&request.address)
        .complete(None)
        .await?;

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
    use crate::scripts::ScriptCache;
    use dotenv::dotenv;

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
        dotenv().ok();

        let request = InternalTransferRequest {
            account: Some(AccountInfo {
                account_id: "6d4cd57d-bf6d-40e5-aabb-ff29d07ebf84".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "96583f8d90dd2fbd6ba72f992576c0906ae6a37072383ce71b815fd6".to_string(),
                is_script_master_key: false,
                operation_key: "0160ee500ac5bff5a353f92701ca1df9de204368cf054039d90b6231".to_string(),
                is_script_operation_key: false,
            }),
            receiver_account: Some(AccountInfo {
                account_id: "2953c409-63d0-4bee-abbe-f11e4c8be0ce".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "75073cd8873918696e373264763422e5c4d4fe63b4e941c42a5381d6".to_string(),
                is_script_master_key: false,
                operation_key: "c472ce633c4acf91dda311f10cb45671d6dcfbae3bdc25d219f83159".to_string(),
                is_script_operation_key: false,
            }),
            to_transfer: vec![Asset {
                unit: "lovelace".to_string(),
                quantity: "10000000".to_string(),
            }],
            address: "addr1qxt9s0udjrwjl0tt5uhejftkczgx4e4rwpers088rwq4l4ke4usmcymrnnjgq22cs7fku6ytpcltxmaly5kvlne0tges00kpag".to_string(),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "74590385acea755d91a0d4c4f4c0fc8d9e1fa0627d33735a95a7f727ca371af6".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr1vxqvl5uxf66cpx89l6965wklka7ucuwxasyxz68dvakmhacjpuz0c".to_string(),
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
            empty_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 893,
                    tx_hash: "74590385acea755d91a0d4c4f4c0fc8d9e1fa0627d33735a95a7f727ca371af6".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr1qxqvl5uxf66cpx89l6965wklka7ucuwxasyxz68dvakmhamdcgjlk94f0wjhvgu8fx3evwzw8dcnretnnj42xyuw7guqpp4d7m".to_string(),
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
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "522ef27241b80a8ea4f54ed5c1674fcf49638c3a4b65a0e21371a21a0d343956".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr1wx73pap60td0s7mfmtlgv74qwru64xlt9g7nlmf5ctjd8aq04h27t".to_string(),
                    amount: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "6000000".to_string(),
                        },
                        Asset {
                            unit: "f9548d81418838f704b9dbd6a172422c943878934756c3fd28d736ef".to_string(),
                            quantity: "1".to_string(),
                        }
                    ],
                    data_hash: "426bcc8154a5320d0e62212c5fe774f5e19c40ba9a18c2355a354997425a121e".to_string(),
                    plutus_data: "d8799f581c80cfd3864eb58098e5fe8baa3adfb77dcc71c6ec086168ed676dbbf7581c9df20621e6bc47694d878e8a10214feb2ab311ef2a0ef634875c7f55d8799fd8799f506f741d20e13c41598215e8b0f762aa76d8799f581c18ab3c3346746ab95f78d3a54c2ddd53a98ffe5c447b85091e336bb9ffd8799f581c4d84776b18d8dcedab1f2b9499c674d50e16d50d1b892d28c55e1da0ffff581cc90820268cd4879e21beabcbf13e992ecdd42ffa88720f4092cf9fa2ff58200000000000000000000000000000000000000000000000000000000000000000581c8516d6c94f15a040537b3f8ac5be09123fe01e02d60dcc8dd1276628d8799fd87a9f581c76453dd95f7a7f1cd95473aae5ace476a847973b1b3dd1547b9af8c4ffd87a80ff581cf9548d81418838f704b9dbd6a172422c943878934756c3fd28d736efd8799fd87a9f581cbd10f43a7adaf87b69dafe867aa070f9aa9beb2a3d3fed34c2e4d3f4ffd87a80ff581c01ea173d0e9d6cfe833789532a8f3b6ab4e5ee1a247e249b51b2a7f6581c467ca8b7dabf347957f6ddeafad7f780487c9260f31f5aaa36001344581cc90820268cd4879e21beabcbf13e992ecdd42ffa88720f4092cf9fa2581c9981a3ebed1dfeccafc0fe97384f5bbb0e6cd856b3c680ba0bb2697eff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
        };

        let config = AppConfig::new();
        let scripts = ScriptCache::new();
        let result = handler(request, &config, &scripts).await;
        println!("Result: {:?}", result);
        assert!(result.is_ok());
    }
}
