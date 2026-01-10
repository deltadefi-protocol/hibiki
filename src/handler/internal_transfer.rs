use hibiki_proto::services::{IntentTxResponse, InternalTransferRequest};
use whisky::{
    calculate_tx_hash, data::PlutusDataJson, Asset, Budget, UTxO, UtxoInput, UtxoOutput, WData,
    WError, WRedeemer,
};

use crate::{
    config::AppConfig,
    constant::{dex_oracle_nft, l2_ref_scripts_index},
    scripts::{
        hydra_user_intent_mint_minting_blueprint, hydra_user_intent_spend_spending_blueprint,
        MasterIntent, MintMasterIntent, TransferIntent, UserTradeAccount,
    },
    utils::{
        hydra::{get_hydra_tx_builder, get_script_ref_hex},
        proto::{assets_to_mvalue, from_proto_amount, from_proto_utxo},
        token::to_hydra_token,
    },
};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub async fn handler(request: InternalTransferRequest) -> Result<IntentTxResponse, WError> {
    let AppConfig { app_owner_vkey, .. } = AppConfig::new();
    let collateral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let empty_utxo = from_proto_utxo(request.empty_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(request.dex_order_book_utxo.as_ref().unwrap());
    let account = request.account.unwrap();

    let mut tx_builder = get_hydra_tx_builder();
    let policy_id = whisky::data::PolicyId::new(dex_oracle_nft());
    let user_intent_mint = hydra_user_intent_mint_minting_blueprint(&policy_id);
    let user_intent_spend = hydra_user_intent_spend_spending_blueprint(&policy_id);

    let from_account = UserTradeAccount::from_proto(&account);
    let to_account = UserTradeAccount::from_proto(&request.receiver_account.unwrap());
    let transfer_amount_l2 =
        assets_to_mvalue(&to_hydra_token(&from_proto_amount(&request.to_transfer)));

    // Create transfer intent
    let hydra_account_intent = TransferIntent::new(to_account.clone(), transfer_amount_l2);
    let redeemer_json = MintMasterIntent::new(from_account.clone(), hydra_account_intent.clone());
    let datum_json = MasterIntent::new(from_account, hydra_account_intent);

    let intent_script_ref_hex = Some(get_script_ref_hex(&user_intent_mint.cbor)?);
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
            script_ref: intent_script_ref_hex,
            script_hash: Some(user_intent_mint.hash.clone()),
        },
    };

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
            user_intent_mint.cbor.len() / 2,
        )
        .input_for_evaluation(&intent_mint_ref_utxo)
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

        unsafe {
            std::env::set_var(
                "DEX_ORACLE_NFT",
                "61cfe987895c09ac73cea4c36472c2b7dc1edddc2952e102468adcad",
            )
        };
        unsafe {
            std::env::set_var(
                "OWNER_VKEY",
                "672dabb562df94a2c363a8fb175225a333bb3225ebe0273b9b7a539c",
            )
        };

        let request = InternalTransferRequest {
            account: Some(AccountInfo {
                account_id: "82d892a6-6366-41c6-b5c8-841d1674b982".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
                is_script_master_key: false,
                operation_key: "70ff52c44582deb40453efb9710cab86fccd75feccbe0c670a600422".to_string(),
                is_script_operation_key: false,
            }),
            receiver_account: Some(AccountInfo {
                account_id: "04099b86-aef9-4600-94a8-4d38f5a94aad".to_string(),
                account_type: "spot_account".to_string(),
                master_key: "af5c5b46fa7b046d4290b2787bd21db57f7b5a3ba6bc7666edb9e9a8".to_string(),
                is_script_master_key: false,
                operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
                is_script_operation_key: false,
            }),
            to_transfer: vec![
                Asset {
                    unit: "a2818ba06a88bb6c08d10f4f9b897c09768f28d274093628ad7086fc484f534b59".to_string(),
                    quantity: "100".to_string(),
                },
                Asset {
                    unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                    quantity: "100000000".to_string(),
                },
                Asset {
                    unit: "lovelace".to_string(),
                    quantity: "100000000".to_string(),
                },
                Asset {
                    unit: "3363b99384d6ee4c4b009068af396c8fdf92dafd111e58a857af04294e49474854".to_string(),
                    quantity: "100000000".to_string(),
                },
                Asset {
                    unit: "378f9732c755ed6f4fc8d406f1461d0cca95d7d2e69416784684df39534e454b".to_string(),
                    quantity: "100".to_string(),
                },
                Asset {
                    unit: "82e46eb16633bf8bfa820c83ffeb63192c6e21757d2bf91290b2f41d494147".to_string(),
                    quantity: "100000000".to_string(),
                },
            ],
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "ce0338045d801531f260e784201cb71cff22965c45f4001c449f33e38248baf8".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1vpnjm2a4vt0efgkrvw50k96jyk3n8wejyh47qfemnda988qm8as9a".to_string(),
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
                    output_index: 570,
                    tx_hash: "ce0338045d801531f260e784201cb71cff22965c45f4001c449f33e38248baf8".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1qpnjm2a4vt0efgkrvw50k96jyk3n8wejyh47qfemnda98886r5k6nf7zgqxm6uxr3f2j8823mh58yln5t65hlsn9kzdqvutf79".to_string(),
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
                    tx_hash: "bdd0b20038334a32106180d90dabf5ec372f25a335a9d906240fc4a87e78a799".to_string(),
                }),
                output: Some(UtxoOutput {
                    address: "addr_test1wpwxfsk0ar3hw5pxt8q2za870ekzf3kxhudkspf3ak37n3ch4cxst".to_string(),
                    amount: vec![
                        Asset {
                            unit: "lovelace".to_string(),
                            quantity: "6000000".to_string(),
                        },
                        Asset {
                            unit: "61cfe987895c09ac73cea4c36472c2b7dc1edddc2952e102468adcad".to_string(),
                            quantity: "1".to_string(),
                        },
                    ],
                    data_hash: "bd088955e755090f3823bb32070d229587e0eac18052745b21bac719a45901e8".to_string(),
                    plutus_data: "d8799f581c672dabb562df94a2c363a8fb175225a333bb3225ebe0273b9b7a539c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f5003558dbdf58640cd8d04404616aa3d08d8799f581c696dcba9c0e4bf93e96147e220cf7955ff88d0a2dcc0fd75b07dbd93ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581cfd4873bc630390af16c5bf391b522ab74239e9830f69665a80f16b68ff58200000000000000000000000000000000000000000000000000000000000000000581cee491e4596c7c19a418452a95e4befe6833778fbd7dd3772375f5450d8799fd87a9f581cdb34d43ad79619b1b51eda82d9faad1d01e70e1af275862f08b8bcedffd87a80ff581c61cfe987895c09ac73cea4c36472c2b7dc1edddc2952e102468adcadd8799fd87a9f581c5c64c2cfe8e377502659c0a174fe7e6c24c6c6bf1b680531eda3e9c7ffd87a80ff581c904d0be26ba2f9612d954ec7fed99db3196926878285a577c7df2ae5581cd3750e4d39eaff1f40599225ecea61b10ab492770d0f54ddae5f800b581cfd4873bc630390af16c5bf391b522ab74239e9830f69665a80f16b68581ca769dcdd8805cd766f25762f74b0eb99f79317a7519fe3091599f559ff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
        };

        let result = handler(request).await;
        println!("Result: {:?}", result);
        assert!(result.is_ok());
    }
}
