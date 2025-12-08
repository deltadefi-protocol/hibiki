use hibiki_proto::services::{CreateHydraAccountUtxoRequest, CreateHydraAccountUtxoResponse};
use whisky::{calculate_tx_hash, data::Value, Asset, Budget, WError, WRedeemer, Wallet};

use crate::{
    config::AppConfig,
    handler::sign_transaction::check_signature_sign_tx,
    scripts::{
        hydra_account_balance_minting_blueprint, hydra_account_balance_spending_blueprint,
        HydraAccountBalanceDatum, MintPolarity, UserAccount,
    },
    utils::{hydra::get_hydra_tx_builder, proto::from_proto_utxo},
};

pub async fn handler(
    request: CreateHydraAccountUtxoRequest,
    app_owner_wallet: &Wallet,
) -> Result<CreateHydraAccountUtxoResponse, WError> {
    let AppConfig { app_owner_vkey, .. } = AppConfig::new();

    let colleteral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(request.dex_order_book_utxo.as_ref().unwrap());
    let empty_utxo = from_proto_utxo(request.empty_utxo.as_ref().unwrap());
    let account = UserAccount::from_proto(request.account.as_ref().unwrap());

    let account_balance_spend = hydra_account_balance_spending_blueprint();
    let account_balance_mint = hydra_account_balance_minting_blueprint();

    let mut tx_builder = get_hydra_tx_builder();
    tx_builder
        // reference oracle utxo
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        // mint the hydra account utxo token
        .mint_plutus_script_v3()
        .mint(1, &account_balance_mint.hash, "")
        .mint_redeemer_value(&WRedeemer {
            data: account_balance_mint.redeemer(MintPolarity::RMint),
            ex_units: Budget::default(),
        })
        .minting_script(&account_balance_mint.cbor)
        // lock it at validator
        .tx_out(
            &account_balance_spend.address,
            &[Asset::new_from_str(&account_balance_mint.hash, "1")],
        )
        .tx_out_inline_datum_value(
            &account_balance_spend.datum(HydraAccountBalanceDatum::Datum(account, Value::new())),
        )
        // empty utxo
        .tx_in(
            &empty_utxo.input.tx_hash,
            empty_utxo.input.output_index,
            &empty_utxo.output.amount,
            &empty_utxo.output.address,
        )
        .input_for_evaluation(&empty_utxo)
        .tx_out(&empty_utxo.output.address, &empty_utxo.output.amount)
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

    Ok(CreateHydraAccountUtxoResponse {
        signed_tx,
        tx_hash,
        account_utxo_tx_index: 0,
        new_empty_utxo_tx_index: 1,
    })
}

// todo: update dexOrderBook datum tests
// #[cfg(test)]
// mod tests {
//     use crate::utils::wallet::get_app_owner_wallet;

//     use super::*;
//     use dotenv::dotenv;
//     use hibiki_proto::services::{AccountInfo, Asset, UTxO, UtxoInput, UtxoOutput};

//     #[tokio::test]
//     async fn test_create_hydra_account_utxo() {
//         dotenv().ok();
//         let request = CreateHydraAccountUtxoRequest {
//             account: Some(AccountInfo {
//                 account_id: "4a3ec060-5156-4321-a180-5924270608cb".to_string(),
//                 account_type: "spot_account".to_string(),
//                 master_key: "fdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4".to_string(),
//                 is_script_master_key: false,
//                 operation_key: "0b01677be7cda9eeb4f5a15c5c14ac9b715d6018a09cb70258093e95".to_string(),
//                 is_script_operation_key: false
//             }),
//             empty_utxo: Some(UTxO {
//                 input: Some(UtxoInput {
//                     output_index: 862,
//                     tx_hash: "89c54b3a70ef3608355bd822b3df44c672150654fd211c8e02e37e1453e04f7d".to_string()
//                 }),
//                 output: Some(UtxoOutput {
//                     address: "addr_test1qra9zdhfa8kteyr3mfe7adkf5nlh8jl5xcg9e7pcp5w9yhyf5tek6vpnha97yd5yw9pezm3wyd77fyrfs3ynftyg7njs5cfz2x".to_string(),
//                     amount: vec![Asset {
//                         unit: "lovelace".to_string(),
//                         quantity: "0".to_string()
//                     }],
//                     data_hash: "".to_string(),
//                     plutus_data: "".to_string(),
//                     script_ref: "".to_string(),
//                     script_hash: "".to_string()
//                 })
//             }),
//             address: "addr_test1qr77kjlsarq8wy22g4flrcznjh5lkug5mvth7qhhkewgmezwvc8hnnjzy82j5twzf8dfy5gjk04yd09t488ys9605dvq4ymc4x".to_string(),
//             collateral_utxo: Some(UTxO {
//                 input: Some(UtxoInput {
//                     output_index: 1000,
//                     tx_hash: "89c54b3a70ef3608355bd822b3df44c672150654fd211c8e02e37e1453e04f7d".to_string()
//                 }),
//                 output: Some(UtxoOutput {
//                     address: "addr_test1vra9zdhfa8kteyr3mfe7adkf5nlh8jl5xcg9e7pcp5w9yhq5exvwh".to_string(),
//                     amount: vec![Asset {
//                         unit: "lovelace".to_string(),
//                         quantity: "10000000".to_string()
//                     }],
//                     data_hash: "".to_string(),
//                     plutus_data: "".to_string(),
//                     script_ref: "".to_string(),
//                     script_hash: "".to_string()
//                 })
//             }),
//             dex_order_book_utxo: Some(UTxO {
//                 input: Some(UtxoInput {
//                     output_index: 0,
//                     tx_hash: "5c86e2e9f538a9dd73da3c9460db330621ded0949aa7d944d697cbb60d46c9b1".to_string()
//                 }),
//                 output: Some(UtxoOutput {
//                     address: "addr_test1wq80wduvwq4s99cqyn0ra7qdpds7gatmxhun8vterp957zse5cnq7".to_string(),
//                     amount: vec![Asset {
//                         unit: "lovelace".to_string(),
//                         quantity: "6000000".to_string()
//                     }, Asset {
//                         unit: "0ae6fc26bbcb00cf8039777488a6b2c2c8ec44b8a16e48b11056e3a3".to_string(),
//                         quantity: "1".to_string()
//                     }],
//                     data_hash: "670ed73b9f3f66eddba1c0a8de8fc003f255d18be213c798cc715c414f021938".to_string(),
//                     plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cbc2a083f306397a56661f0d999be07faeb39b792ae97ece7736ef2c3d8799fd8799f507c2f29ff88294a8393180c32ae6bbb4fd8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cff850ae069dfe1aef8db8b8fb40fd9b6d2c7f4d61b5bb7e5ad3fd3b6ffffff58200000000000000000000000000000000000000000000000000000000000000000581c618b0aca12eac0ff0dc5886503bf347471908b168a90b0e25692caa4d8799fd87a9f581c2cacd4b1e8d4788e46747e454a7f42978289de7f9ccb5812b1089c87ffd87a80ff581c6cd00ea1f6c2e96d20fc6beda30db317b8a972e12f97c8c80032b718d8799fd87a9f581c0c6a291cd4d28f5cec51e5ef7b504ce5476b19f7c97a75ac3e1a5be4ffd87a80ff581c0ae6fc26bbcb00cf8039777488a6b2c2c8ec44b8a16e48b11056e3a3d8799fd87a9f581c0ef7378c702b02970024de3ef80d0b61e4757b35f933b179184b4f0affd87a80ff581c463e70d04718e253757523698184cb7090b0430e89dc025c4c8e392cd8799fd87a9f581ceb0a5938244e92fd172560f530bf959724b10353a26f276ea8bbb3ccffd87a80ff581cc828db378a1b202822e9de2a6d461af04b016768bce986176af87ba5d8799fd87a9f581c1318e21d5eb0eb93f23a4d9a52592db44dd48e971fe6de91a8c14071ffd87a80ff581c84dc6d6189703f72bb0d1f8bd34ea1fb4d30125490f74d90d8e443e4d8799fd87a9f581c1d050750038d16006c12ab6f43c0233acaae7e5511ea7cea2638ac30ffd87a80ffd8799f581cb64c1a7aa350783dc615649fe86d9ffd31fe1fc3ae62e4d4292a5eb0581cd8d03140f553bdb5b2abcd8615e1ab6214cf44680cb98efd35fbb25b581c3e8df27e4d1a20debb94b67be21a4df333b1c2daf83ea4d2ddd15cb8581c4442f121ae5abdc9d801053ebaed381cabd12ec8b251f19f4e4d1271581c9039e718c927ba664c15e7ac12c1aa2ab8bf87ca3da5a5414537e003581ce62fc27cad55eaeafb4a790eed11662f40a92d8729f3b95ad68acb97581c0b970000e9ab147200682560ae44ef62d484f019c9cbece2515872ac581c2196da87e927a233737b5a0974ec64d441c12825712861ae680fefdb581c1dc6690b19d6c0128fd8a206634d62ec41cfa1c58dbcfb96ae3bbaee581ced3f053ef237c6d559f218f8342a490ee68fdbb5a0fde6c64b83e8f8ffff".to_string(),
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
