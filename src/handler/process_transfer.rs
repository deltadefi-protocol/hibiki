use hibiki_proto::services::{ProcessTransferRequest, ProcessTransferResponse};
use whisky::{
    calculate_tx_hash,
    data::{Constr0, Value},
    Budget, WError, WRedeemer, Wallet,
};

use crate::{
    config::AppConfig,
    handler::sign_transaction::check_signature_sign_tx,
    scripts::{
        hydra_account_balance_spending_blueprint, hydra_internal_transfer_blueprint,
        hydra_user_intent_minting_blueprint, hydra_user_intent_spending_blueprint,
        HydraAccountBalanceDatum, HydraAccountBalanceRedeemer, HydraUserIntentRedeemer,
        UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{from_proto_balance_utxo, from_proto_utxo},
    },
};

pub async fn handler(
    request: ProcessTransferRequest,
    app_owner_wallet: &Wallet,
) -> Result<ProcessTransferResponse, WError> {
    let AppConfig { app_owner_vkey, .. } = AppConfig::new();

    let colleteral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(request.dex_order_book_utxo.as_ref().unwrap());
    let intent_utxo = from_proto_utxo(request.transferral_intent_utxo.as_ref().unwrap());
    let from_account = UserAccount::from_proto(request.account.as_ref().unwrap());
    let to_account = UserAccount::from_proto(request.receiver_account.as_ref().unwrap());
    let (from_updated_balance, from_account_utxo) =
        from_proto_balance_utxo(request.account_balance_utxo.as_ref().unwrap());
    let (to_updated_balance, to_account_utxo) =
        from_proto_balance_utxo(request.receiver_account_balance_utxo.as_ref().unwrap());

    let user_intent_mint = hydra_user_intent_minting_blueprint();
    let user_intent_spend = hydra_user_intent_spending_blueprint();
    let account_balance_spend = hydra_account_balance_spending_blueprint();
    let internal_transfer_withdraw = hydra_internal_transfer_blueprint();

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
            data: user_intent_spend.redeemer(Constr0::new(())),
            ex_units: Budget::default(),
        })
        .tx_in_script(&user_intent_spend.cbor)
        .input_for_evaluation(&intent_utxo)
        // update account balance utxo
        .spending_plutus_script_v3()
        .tx_in(
            &from_account_utxo.input.tx_hash,
            from_account_utxo.input.output_index,
            &from_account_utxo.output.amount,
            &from_account_utxo.output.address,
        )
        .tx_in_inline_datum_present()
        .tx_in_redeemer_value(&WRedeemer {
            data: account_balance_spend
                .redeemer(HydraAccountBalanceRedeemer::UpdateBalanceWithTransfer),
            ex_units: Budget::default(),
        })
        .tx_in_script(&user_intent_spend.cbor)
        .input_for_evaluation(&from_account_utxo)
        .tx_out(
            &from_account_utxo.output.address,
            &from_account_utxo.output.amount,
        )
        .tx_out_inline_datum_value(
            &account_balance_spend.datum(HydraAccountBalanceDatum::Datum(
                from_account,
                Value::from_asset_vec(&from_updated_balance),
            )),
        )
        // update receiver account balance utxo
        .spending_plutus_script_v3()
        .tx_in(
            &to_account_utxo.input.tx_hash,
            to_account_utxo.input.output_index,
            &to_account_utxo.output.amount,
            &to_account_utxo.output.address,
        )
        .tx_in_inline_datum_present()
        .tx_in_redeemer_value(&WRedeemer {
            data: account_balance_spend
                .redeemer(HydraAccountBalanceRedeemer::UpdateBalanceWithTransfer),
            ex_units: Budget::default(),
        })
        .tx_in_script(&account_balance_spend.cbor)
        .input_for_evaluation(&to_account_utxo)
        .tx_out(
            &to_account_utxo.output.address,
            &to_account_utxo.output.amount,
        )
        .tx_out_inline_datum_value(
            &account_balance_spend.datum(HydraAccountBalanceDatum::Datum(
                to_account,
                Value::from_asset_vec(&to_updated_balance),
            )),
        )
        .mint_plutus_script_v3()
        .mint(-1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&WRedeemer {
            data: user_intent_mint.redeemer(HydraUserIntentRedeemer::HydraUserTransfer),
            ex_units: Budget::default(),
        })
        .minting_script(&user_intent_mint.cbor)
        // withdrawal logic
        .withdrawal_plutus_script_v3()
        .withdrawal(&internal_transfer_withdraw.address, 0)
        .withdrawal_redeemer_value(&WRedeemer {
            data: user_intent_spend.redeemer(Constr0::new(())),
            ex_units: Budget::default(),
        })
        .withdrawal_script(&internal_transfer_withdraw.cbor)
        // common
        .tx_in_collateral(
            &colleteral.input.tx_hash,
            colleteral.input.output_index,
            &colleteral.output.amount,
            &colleteral.output.address,
        )
        .input_for_evaluation(&colleteral)
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
        account_utxo_tx_index: 0,
        receiver_account_utxo_tx_index: 1,
    })
}

// todo: update dexOrderBook datum tests
// #[cfg(test)]
// mod tests {
//     use crate::utils::wallet::get_app_owner_wallet;

//     use super::*;
//     use dotenv::dotenv;
//     use hibiki_proto::services::{AccountInfo, Asset, UTxO, UtxoInput, UtxoOutput};
//     use hibiki_proto::services::{BalanceUtxo, ProcessTransferRequest};

//     #[tokio::test]
//     async fn test_process_transfer() {
//         dotenv().ok();
//         let request = ProcessTransferRequest {
//             account: Some(AccountInfo {
//                 account_id: "6238ab50-164a-444f-8f99-1c44c93e998f".to_string(),
//                 account_type: "spot_account".to_string(),
//                 master_key: "04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66".to_string(),
//                 is_script_master_key: false,
//                 operation_key: "1d585b0cc5ae93d6c65bc72773ca5f77855f1e658e969c70f0fb3cd4".to_string(),
//                 is_script_operation_key: false
//             }),
//             receiver_account: Some(AccountInfo {
//                 account_id: "b897f81c-68ca-41aa-a8df-438fa7e0ee57".to_string(),
//                 account_type: "spot_account".to_string(),
//                 master_key: "948624fda9524c4e8531e40371afa4e4a326f005da065b43312f7ceb".to_string(),
//                 is_script_master_key: false,
//                 operation_key: "a59998c27b9bb8534a7948c67421c16331d4115ae7f965bb660c405b".to_string(),
//                 is_script_operation_key: false
//             }),
//             account_balance_utxo: Some(BalanceUtxo {
//                 utxo: Some(UTxO {
//                     input: Some(UtxoInput {
//                         output_index: 2,
//                         tx_hash: "0f6569f7fa0419c170d32b5a4cb98204970b069f64d1c7cbb4935e0b438f4011".to_string()
//                     }),
//                     output: Some(UtxoOutput {
//                         address: "addr_test1wqf33csat6cwhylj8fxe55je9k6ym4ywju07dh534rq5qug84wcud".to_string(),
//                         amount: vec![
//                             Asset {
//                                 unit: "lovelace".to_string(),
//                                 quantity: "0".to_string()
//                             },
//                             Asset {
//                                 unit: "c828db378a1b202822e9de2a6d461af04b016768bce986176af87ba5".to_string(),
//                                 quantity: "1".to_string()
//                             }
//                         ],
//                         data_hash: "6af47713a4b3b285bbb078d7877c918fe4351da95ecc024e09e71df59a641e6a".to_string(),
//                         plutus_data: "d8799fd8799fd8799f506238ab50164a444f8f991c44c93e998fd8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581c1d585b0cc5ae93d6c65bc72773ca5f77855f1e658e969c70f0fb3cd4ffffffa240a1401a00989680581c5066154a102ee037390c5236f78db23239b49c5748d3d349f3ccf04ba144555344581a00989680ff".to_string(),
//                         script_ref: "".to_string(),
//                         script_hash: "".to_string()
//                     })
//                 }),
//                 updated_balance: vec![
//                     Asset {
//                         unit: "lovelace".to_string(),
//                         quantity: "9000000".to_string()
//                     },
//                     Asset {
//                         unit: "5066154a102ee037390c5236f78db23239b49c5748d3d349f3ccf04b55534458".to_string(),
//                         quantity: "10000000".to_string()
//                     }
//                 ]
//             }),
//             receiver_account_balance_utxo: Some(BalanceUtxo {
//                 utxo: Some(UTxO {
//                     input: Some(UtxoInput {
//                         output_index: 0,
//                         tx_hash: "7c444ed8836d7047d5e41e15912777f756633208b0c890cd75ad29b4c3287f23".to_string()
//                     }),
//                     output: Some(UtxoOutput {
//                         address: "addr_test1wqf33csat6cwhylj8fxe55je9k6ym4ywju07dh534rq5qug84wcud".to_string(),
//                         amount: vec![
//                             Asset {
//                                 unit: "lovelace".to_string(),
//                                 quantity: "0".to_string()
//                             },
//                             Asset {
//                                 unit: "c828db378a1b202822e9de2a6d461af04b016768bce986176af87ba5".to_string(),
//                                 quantity: "1".to_string()
//                             }
//                         ],
//                         data_hash: "7ecc7913eed31c8dcc31a6c69be81cee1f3d8b2b92c6fe65d29893462ca64bee".to_string(),
//                         plutus_data: "d8799fd8799fd8799f50b897f81c68ca41aaa8df438fa7e0ee57d8799f581c948624fda9524c4e8531e40371afa4e4a326f005da065b43312f7cebffd8799f581ca59998c27b9bb8534a7948c67421c16331d4115ae7f965bb660c405bffffffa0ff".to_string(),
//                         script_ref: "".to_string(),
//                         script_hash: "".to_string()
//                     })
//                 }),
//                 updated_balance: vec![
//                     Asset {
//                         unit: "lovelace".to_string(),
//                         quantity: "1000000".to_string()
//                     }
//                 ]
//             }),
//             transferral_intent_utxo: Some(UTxO {
//                 input: Some(UtxoInput {
//                     output_index: 0,
//                     tx_hash: "b9599f3bfa626542f5520715d507f78c57d9f0fe6928416d27d067426241d741".to_string()
//                 }),
//                 output: Some(UtxoOutput {
//                     address: "addr_test1wqf33csat6cwhylj8fxe55je9k6ym4ywju07dh534rq5qug84wcud".to_string(),
//                     amount: vec![
//                         Asset {
//                             unit: "lovelace".to_string(),
//                             quantity: "0".to_string()
//                         },
//                         Asset {
//                             unit: "463e70d04718e253757523698184cb7090b0430e89dc025c4c8e392c".to_string(),
//                             quantity: "1".to_string()
//                         }
//                     ],
//                     data_hash: "40c19dfbf2514c3b93e1d60bbfe03b70e617b0b7976170757d957e7c1f9a84eb".to_string(),
//                     plutus_data: "d87c9fd8799fd8799f506238ab50164a444f8f991c44c93e998fd8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581c1d585b0cc5ae93d6c65bc72773ca5f77855f1e658e969c70f0fb3cd4ffffffd8799fd8799f50b897f81c68ca41aaa8df438fa7e0ee57d8799f581c948624fda9524c4e8531e40371afa4e4a326f005da065b43312f7cebffd8799f581ca59998c27b9bb8534a7948c67421c16331d4115ae7f965bb660c405bffffffa140a1401a000f4240ff".to_string(),
//                     script_ref: "".to_string(),
//                     script_hash: "".to_string()
//                 })
//             }),
//             address: "addr_test1qqzgg5pcaeyea69uptl9da5g7fajm4m0yvxndx9f4lxpkehqgezy0s04rtdwlc0tlvxafpdrfxnsg7ww68ge3j7l0lnszsw2wt".to_string(),
//             collateral_utxo: Some(UTxO {
//                 input: Some(UtxoInput {
//                     output_index: 1000,
//                     tx_hash: "df50c3d4b115012ca00bb163f0492b1b67e53ac0117bfa86cc9e54cc24770787".to_string()
//                 }),
//                 output: Some(UtxoOutput {
//                     address: "addr_test1vra9zdhfa8kteyr3mfe7adkf5nlh8jl5xcg9e7pcp5w9yhq5exvwh".to_string(),
//                     amount: vec![
//                         Asset {
//                             unit: "lovelace".to_string(),
//                             quantity: "10000000".to_string()
//                         }
//                     ],
//                     data_hash: "".to_string(),
//                     plutus_data: "".to_string(),
//                     script_ref: "".to_string(),
//                     script_hash: "".to_string()
//                 })
//             }),
//             dex_order_book_utxo: Some(UTxO {
//                 input: Some(UtxoInput {
//                     output_index: 0,
//                     tx_hash: "f5e3e101088e67685668587aae88a527586bdece2e69a1c683b4c1d8201e07d6".to_string()
//                 }),
//                 output: Some(UtxoOutput {
//                     address: "addr_test1wq80wduvwq4s99cqyn0ra7qdpds7gatmxhun8vterp957zse5cnq7".to_string(),
//                     amount: vec![
//                         Asset {
//                             unit: "lovelace".to_string(),
//                             quantity: "6000000".to_string()
//                         },
//                         Asset {
//                             unit: "0ae6fc26bbcb00cf8039777488a6b2c2c8ec44b8a16e48b11056e3a3".to_string(),
//                             quantity: "1".to_string()
//                         }
//                     ],
//                     data_hash: "2026506915b0afdef9723673b5ca604f5b7b700f75635f8c6869a8af2b471ed5".to_string(),
//                     plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cbc2a083f306397a56661f0d999be07faeb39b792ae97ece7736ef2c3d8799fd8799f506238ab50164a444f8f991c44c93e998fd8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581c1d585b0cc5ae93d6c65bc72773ca5f77855f1e658e969c70f0fb3cd4ffffff58200000000000000000000000000000000000000000000000000000000000000000581c618b0aca12eac0ff0dc5886503bf347471908b168a90b0e25692caa4d8799fd87a9f581c2cacd4b1e8d4788e46747e454a7f42978289de7f9ccb5812b1089c87ffd87a80ff581c6cd00ea1f6c2e96d20fc6beda30db317b8a972e12f97c8c80032b718d8799fd87a9f581c0c6a291cd4d28f5cec51e5ef7b504ce5476b19f7c97a75ac3e1a5be4ffd87a80ff581c0ae6fc26bbcb00cf8039777488a6b2c2c8ec44b8a16e48b11056e3a3d8799fd87a9f581c0ef7378c702b02970024de3ef80d0b61e4757b35f933b179184b4f0affd87a80ff581c463e70d04718e253757523698184cb7090b0430e89dc025c4c8e392cd8799fd87a9f581ceb0a5938244e92fd172560f530bf959724b10353a26f276ea8bbb3ccffd87a80ff581cc828db378a1b202822e9de2a6d461af04b016768bce986176af87ba5d8799fd87a9f581c1318e21d5eb0eb93f23a4d9a52592db44dd48e971fe6de91a8c14071ffd87a80ff581c84dc6d6189703f72bb0d1f8bd34ea1fb4d30125490f74d90d8e443e4d8799fd87a9f581c1d050750038d16006c12ab6f43c0233acaae7e5511ea7cea2638ac30ffd87a80ffd8799f581cb64c1a7aa350783dc615649fe86d9ffd31fe1fc3ae62e4d4292a5eb0581cd8d03140f553bdb5b2abcd8615e1ab6214cf44680cb98efd35fbb25b581c3e8df27e4d1a20debb94b67be21a4df333b1c2daf83ea4d2ddd15cb8581c4442f121ae5abdc9d801053ebaed381cabd12ec8b251f19f4e4d1271581c9039e718c927ba664c15e7ac12c1aa2ab8bf87ca3da5a5414537e003581ce62fc27cad55eaeafb4a790eed11662f40a92d8729f3b95ad68acb97581c0b970000e9ab147200682560ae44ef62d484f019c9cbece2515872ac581c2196da87e927a233737b5a0974ec64d441c12825712861ae680fefdb581c1dc6690b19d6c0128fd8a206634d62ec41cfa1c58dbcfb96ae3bbaee581ced3f053ef237c6d559f218f8342a490ee68fdbb5a0fde6c64b83e8f8ffff".to_string(),
//                     script_ref: "".to_string(),
//                     script_hash: "".to_string()
//                 })
//             })
//         };

//         let app_owner_wallet = get_app_owner_wallet();
//         let result = handler(request, &app_owner_wallet).await;
//         println!("Result: {:?}", result);
//         assert!(result.is_ok());
//     }
// }
