use std::time::Instant;

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

    let start = Instant::now();
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

    log::debug!(
        "[PLACE_ORDER] Intent: {:?}",
        intent.clone().to_json_string()
    );

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

    log::info!(
        "[PLACE_ORDER] tx_hash: {} completed in {:?}",
        tx_hash,
        start.elapsed()
    );

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
    use crate::scripts::{ScriptCache, UserAccount};
    use crate::test_fixtures::{build_collateral_utxo, build_dex_order_book_utxo, L2ScriptConfig};
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
                rt.block_on(test_place_order_case());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn test_place_order_case() {
        init_test_env();

        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();

        let account = AccountInfo {
            account_id: "a810ec7a-7069-4157-aead-48ecf4d693c8".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "fdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        // Create user account for building dex_order_book_utxo
        let user_account = UserAccount::from_proto_trade_account(
            &account,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Build dex_order_book_utxo dynamically with current script hashes
        let dex_order_book_utxo = build_dex_order_book_utxo(
            &scripts,
            &l2_config,
            "4a50e9271bc74e8f143207134a5a62db89fab76eb55fbd00786487966158d86d",
            0,
            &user_account,
        )
        .expect("Failed to build dex_order_book_utxo");

        let request = PlaceOrderRequest {
            address: "addr_test1qr77kjlsarq8wy22g4flrcznjh5lkug5mvth7qhhkewgmezwvc8hnnjzy82j5twzf8dfy5gjk04yd09t488ys9605dvq4ymc4x".to_string(),
            account: Some(account),
            collateral_utxo: Some(build_collateral_utxo(
                "268896892a4e5d6fb004b235be696a990073e4984772eac156da83772a6ce176",
                0,
                "10000000",
            )),
            order_id: "7398aea0-392e-4198-8dc2-4abaf5e5afa4".to_string(),
            is_buy: true,
            list_price_times_one_tri: 890900000000,
            order_size: 199999922800,
            commission_rate_bp: 10,
            empty_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 642,
                    tx_hash: "268896892a4e5d6fb004b235be696a990073e4984772eac156da83772a6ce176".to_string(),
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
            authorized_account_value_l1: vec![Asset {
                unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
                quantity: "199999922800".to_string(),
            }],
            base_token_unit: "lovelace".to_string(),
            quote_token_unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
            order_type: OrderType::Limit as i32,
        };

        let config = AppConfig::new();

        let result = handler(request, &config, &scripts).await;

        match result {
            Ok(response) => {
                println!("=== Place Order Result ===");
                println!("Tx Hash: {}", response.tx_hash);
                println!("Tx Index: {}", response.tx_index);
                println!(
                    "New Empty UTxO Tx Index: {}",
                    response.new_empty_utxo_tx_index
                );
                println!("Tx Hex: {}", response.tx_hex);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }

    #[test]
    fn test_place_order_handler_market_sell() {
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(test_place_order_case_market_sell());
            })
            .unwrap();

        handle.join().unwrap();
    }

    async fn test_place_order_case_market_sell() {
        init_test_env();

        let scripts = ScriptCache::new();
        let l2_config = L2ScriptConfig::default();

        let account = AccountInfo {
            account_id: "a810ec7a-7069-4157-aead-48ecf4d693c8".to_string(),
            account_type: "spot_account".to_string(),
            master_key: "fdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4".to_string(),
            is_script_master_key: false,
            operation_key: "b21f857716821354725bc2bd255dc2e5d5fdfa202556039b76c080a5".to_string(),
            is_script_operation_key: false,
        };

        // Create user account for building dex_order_book_utxo
        let user_account = UserAccount::from_proto_trade_account(
            &account,
            &scripts.hydra_order_book_withdrawal.hash,
        );

        // Build dex_order_book_utxo dynamically with current script hashes
        let dex_order_book_utxo = build_dex_order_book_utxo(
            &scripts,
            &l2_config,
            "4a50e9271bc74e8f143207134a5a62db89fab76eb55fbd00786487966158d86d",
            0,
            &user_account,
        )
        .expect("Failed to build dex_order_book_utxo");

        let request = PlaceOrderRequest {
            address: "addr_test1qr77kjlsarq8wy22g4flrcznjh5lkug5mvth7qhhkewgmezwvc8hnnjzy82j5twzf8dfy5gjk04yd09t488ys9605dvq4ymc4x".to_string(),
            account: Some(account),
            collateral_utxo: Some(build_collateral_utxo(
                "268896892a4e5d6fb004b235be696a990073e4984772eac156da83772a6ce176",
                0,
                "10000000",
            )),
            order_id: "279aee26-ad29-4a00-a47a-fd2acea34e98".to_string(),
            is_buy: false,
            list_price_times_one_tri: 890900000000,
            order_size: 3850000000,
            commission_rate_bp: 10,
            empty_utxo: Some(UTxO {
                input: Some(UtxoInput {
                    output_index: 314,
                    tx_hash: "268896892a4e5d6fb004b235be696a990073e4984772eac156da83772a6ce176".to_string(),
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
            authorized_account_value_l1: vec![Asset {
                unit: "lovelace".to_string(),
                quantity: "3850000000".to_string(),
            }],
            base_token_unit: "lovelace".to_string(),
            quote_token_unit: "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d".to_string(),
            order_type: OrderType::Market as i32,
        };

        let config = AppConfig::new();

        let result = handler(request, &config, &scripts).await;

        match result {
            Ok(response) => {
                println!("=== Place Order Result (Market Sell) ===");
                println!("Tx Hash: {}", response.tx_hash);
                println!("Tx Index: {}", response.tx_index);
                println!(
                    "New Empty UTxO Tx Index: {}",
                    response.new_empty_utxo_tx_index
                );
                println!("Tx Hex: {}", response.tx_hex);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
}
