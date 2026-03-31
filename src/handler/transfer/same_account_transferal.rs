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
    use crate::test_fixtures::{
        build_account_balance_utxo, build_collateral_utxo, build_dex_order_book_utxo,
        L2ScriptConfig,
    };
    use crate::test_utils::init_test_env;
    use crate::utils::wallet::get_app_owner_wallet;
    use hibiki_proto::services::{AccountInfo, Asset, BalanceUtxos};

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

        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();

        let account_info = AccountInfo {
            account_id: "326b8949-4e38-47c2-8887-23e7c5d3d654".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };
        let user_account = UserAccount::from_proto_trade_account(
            &account_info,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Build dex_order_book_utxo dynamically
        let dex_order_book_utxo = build_dex_order_book_utxo(
            &scripts,
            &l2_config,
            "92618472886c4d9c90b39d700371a97aa1164ac8103609577035e96f7791998c",
            0,
            &user_account,
        )
        .expect("Failed to build dex_order_book_utxo");

        // Build account balance UTxOs dynamically
        // All UTxOs have the same L2 token (USDM) but different quantities
        let hydra_token_hash = &scripts.hydra_token_mint.hash;
        // USDM L2 token name (hash of c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d)
        let usdm_l2_name = "ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e";
        let l2_unit = format!("{}{}", hydra_token_hash, usdm_l2_name);

        // (tx_hash, output_index, quantity)
        let balance_utxo_data = vec![
            ("eb22bac72f0c70b20a3f5f4ba958c46f7af00c1a886b94e73cae8d62193bb979", 2u32, "1321"),
            ("4e9644edf42b5c57e15ad1e4fb8e0e7b834223b648d6cc267c2953e680c1f24b", 4, "1745"),
            ("5fe49cf6ebccb104c4072bc83bcfcc4ef53a389bb99285eb1ac242d76039570b", 4, "1211"),
            ("45a7cae12dfc8c5c183d882de265adab5f47436c89b952e9a407fb7a9a81dae8", 4, "1235"),
            ("c08f45e13eebdb9657612066c69181ee41975b9f9ed4a39b3cb00274f13d6e43", 4, "1024"),
            ("b1489b2877a4305c91deb3bf56ea871db390571622cbf87199cc36f4bef4b786", 4, "1606"),
            ("e4341bf07f63dbe21e236655926063e1074da3987a986ac249f27ccee50e1875", 4, "683"),
            ("abbc606ff1d23538b5740883ea415340ce4fc4ab37fbca9593dafbea26c889d8", 4, "1838"),
        ];

        let mut account_balance_utxos = Vec::new();
        for (tx_hash, output_index, quantity) in &balance_utxo_data {
            let balance_asset = Asset {
                unit: l2_unit.clone(),
                quantity: quantity.to_string(),
            };
            let utxo = build_account_balance_utxo(
                &scripts,
                tx_hash,
                *output_index,
                &user_account,
                &balance_asset,
            )
            .expect("Failed to build account_balance_utxo");
            account_balance_utxos.push(utxo);
        }

        let request = SameAccountTransferalRequest {
            address: "addr_test1qqzgg5pcaeyea69uptl9da5g7fajm4m0yvxndx9f4lxpkehqgezy0s04rtdwlc0tlvxafpdrfxnsg7ww68ge3j7l0lnszsw2wt".to_string(),
            account: Some(account_info),
            collateral_utxo: Some(build_collateral_utxo(
                "91fddaab7baf528c4c67c67da8bf20e1de482037b78fb836963de24fdee3d45f",
                0,
                "10000000",
            )),
            account_balance_utxos: Some(BalanceUtxos {
                utxos: account_balance_utxos,
                updated_balance_l1: vec![
                    Asset {
                        unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                        quantity: "10663".to_string(),
                    },
                ],
            }),
            dex_order_book_utxo: Some(dex_order_book_utxo),
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
