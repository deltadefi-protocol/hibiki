use hibiki_proto::services::{IntentTxResponse, OrderType, PlaceOrderRequest};
use whisky::{calculate_tx_hash, data::PlutusDataJson, Asset, WData, WError};

use crate::{
    config::AppConfig,
    scripts::{
        HydraOrderBookIntent, HydraUserIntentDatum, HydraUserIntentRedeemer, ScriptCache,
        UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        order::to_order_datum,
        proto::{assets_to_mvalue, from_proto_amount, from_proto_utxo},
        token::{to_hydra_token, to_hydra_unit},
    },
};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub async fn handler(
    request: PlaceOrderRequest,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<IntentTxResponse, WError> {
    let PlaceOrderRequest {
        address,
        account,
        collateral_utxo,
        order_id,
        is_buy,
        list_price_times_one_tri,
        order_size,
        commission_rate_bp,
        empty_utxo,
        dex_order_book_utxo,
        authorized_account_value_l1,
        base_token_unit,
        quote_token_unit,
        order_type,
    } = request;

    let app_owner_vkey = &config.app_owner_vkey;
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let account = account.unwrap();
    let collateral = from_proto_utxo(collateral_utxo.as_ref().unwrap());
    let empty_utxo = from_proto_utxo(empty_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(dex_order_book_utxo.as_ref().unwrap());
    let user_account = UserAccount::from_proto_trade_account(&account, account_ops_script_hash);

    let mut tx_builder = get_hydra_tx_builder();
    let user_intent_mint = &scripts.user_intent_mint;
    let user_intent_spend = &scripts.user_intent_spend;

    // Create place order intent
    let order_type = OrderType::try_from(order_type).unwrap_or(OrderType::Limit);
    let order_datum = to_order_datum(
        &order_id,
        &to_hydra_unit(&base_token_unit),
        &to_hydra_unit(&quote_token_unit),
        is_buy,
        list_price_times_one_tri,
        order_size,
        commission_rate_bp,
        &user_account,
        order_type,
    );
    let place_order_intent = HydraOrderBookIntent::PlaceOrderIntent(Box::new((
        order_datum,
        assets_to_mvalue(&to_hydra_token(&from_proto_amount(
            &authorized_account_value_l1,
        ))),
    )));
    let intent = Box::new((user_account, place_order_intent));

    println!("Place Order Intent: {:?}", intent.clone().to_json_string());

    tx_builder
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        .mint_plutus_script_v3()
        .mint(1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&user_intent_mint.redeemer(
            HydraUserIntentRedeemer::MintTradeIntent(intent.clone()),
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
            HydraUserIntentDatum::TradeIntent(intent).to_json_string(),
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
        .required_signer_hash(&account.operation_key)
        .tx_in_collateral(
            &collateral.input.tx_hash,
            collateral.input.output_index,
            &collateral.output.amount,
            &collateral.output.address,
        )
        .input_for_evaluation(&collateral)
        .change_address(&address)
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
    use super::*;
    use crate::config::AppConfig;
    use crate::scripts::ScriptCache;
    use crate::test_utils::init_test_env;
    use hibiki_proto::services::{AccountInfo, Asset, UTxO, UtxoInput, UtxoOutput};

    #[test]
    fn test_place_order_handler() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(test_place_order_case_1());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn test_place_order_case_1() {
        init_test_env();

        let account = AccountInfo {
            account_id: "569c4c28-6389-40ac-aa30-e573f8969f09".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "4ba6dd244255995969d2c05e323686bcbaba83b736e729941825d79b".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        let request = PlaceOrderRequest {
            address: "addr_test1qp96dhfygf2ejktf6tq9uv3ks67t4w5rkumww2v5rqja0xcx8ls6mu88ytwql66750at9at4apy4jdezhu22artnvlys7ec2gm".to_string(),
            account: Some(account),
            collateral_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 0,
                    tx_hash: "c5a73bfd3a4b6ade924a2179c0ebe625a0b0529a2650eaabeeb83dd62b56ffd1".to_string(),
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
            order_id: "15f94aee-8c88-4fe6-9680-580f284367d9".to_string(),
            is_buy: true,
            list_price_times_one_tri: 438900000000,
            order_size: 8778000,
            commission_rate_bp: 10,
            empty_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 656,
                    tx_hash: "c5a73bfd3a4b6ade924a2179c0ebe625a0b0529a2650eaabeeb83dd62b56ffd1".to_string(),
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
                    tx_hash: "23df334ce4ac85f4d6ea3468439f87ec907f9d6df8f595d1751acd0f4591ce60".to_string(),
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
                    data_hash: "db15212723f8ecfd6fbaf0a7a52ccee752f164f2dfda685fa673b4de6db3d6c7".to_string(),
                    plutus_data: "d8799f581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581cc25ead27ea81d621dfb7c02dfda90264c5f4777e1e745f96c36aaa15d8799fd8799f5019fb5dfe07d045719104d39e9a0bf8b0d8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581cb21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5ffff581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2ff58200000000000000000000000000000000000000000000000000000000000000000581ce1808a4ae0d35578a215cd68cf63b86ee40759650ea4cde97fc8a05dd8799fd87a9f581cda2156330d5ac0c69125eea74b41e58dd14a80a78b71e7b9add8eb4effd87a80ff581c9ee27af30bcbcf1a399bfa531f5d9aef63f18c9ea761d5ce96ab3d6dd8799fd87a9f581cf0d0af220ab0d0cea8dbf1e51bf260a48674299cd8b8323517329c94ffd87a80ff581c333a05dd70f3eddbf56d5441d75e8a513c6baee7aebe5057351ae85f581cbef30f7146f3370715356f4f88c64928d62708afec6796ddf1070b88581c832b66dd9fa4fddab9d76b47a9e6f9a2b538c053e3a0b42d347a12e2581cb28603ecb7ab3818bac7dc5f7f9260652443bbc1a471afb90c7fc816ff".to_string(),
                    script_ref: "".to_string(),
                    script_hash: "".to_string(),
                }),
            }),
            authorized_account_value_l1: vec![Asset {
                unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                quantity: "8778000".to_string(),
            }],
            base_token_unit: "lovelace".to_string(),
            quote_token_unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
            order_type: OrderType::Limit as i32,
        };

        let config = AppConfig::new();
        let scripts = ScriptCache::new();

        let result = handler(request, &config, &scripts).await;

        match result {
            Ok(response) => {
                println!("=== Place Order Result ===");
                println!("Tx Hash: {}", response.tx_hash);
                println!("Tx Hash: {}", response.tx_hash);
                println!("Tx Index: {}", response.tx_index);
                println!(
                    "New Empty UTxO Tx Index: {}",
                    response.new_empty_utxo_tx_index
                );
                println!("Tx Hex Length: {}", response.tx_hex.len());
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
}
