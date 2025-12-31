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
                "9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6d",
            )
        };
        unsafe {
            std::env::set_var(
                "OWNER_VKEY",
                "fa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c",
            )
        };

        let request = InternalTransferRequest {
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
            to_transfer: vec![
                Asset {
                    unit: "lovelace".to_string(),
                    quantity: "10000000".to_string(),
                },
                Asset {
                    unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                    quantity: "10000000".to_string(),
                },
            ],
            address: "addr_test1qr77kjlsarq8wy22g4flrcznjh5lkug5mvth7qhhkewgmezwvc8hnnjzy82j5twzf8dfy5gjk04yd09t488ys9605dvq4ymc4x".to_string(),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "e4d920a04b1f6197cc9565c30ce33bd3805f8805ca7b6b1802ef369c229c0dca".to_string(),
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
            empty_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 549,
                    tx_hash: "e4d920a04b1f6197cc9565c30ce33bd3805f8805ca7b6b1802ef369c229c0dca".to_string(),
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
            dex_order_book_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "0a730cdc62c8f28d78930a9d9e40991a4fcc5611b1b7b0b85e88c1a502a82d92".to_string(),
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
                    data_hash: "82d061db86c6197e533203d4c142bbacdb4fc5b0ea2d32ad2762a166dd1d4cad".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525cd8799fd8799f50459044917ccb444cbb2343c1ae02016ad8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c2cedf51118d0e78d46062fd4be09e625e3b3a0cb78881639b5807a91ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581c0a4af222798c805464ec76ec9f837a7829a6b07b54953eb8c38db405581c2cedf51118d0e78d46062fd4be09e625e3b3a0cb78881639b5807a91581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
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
