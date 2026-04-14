use hibiki_proto::services::{BurnExpiredIntentsRequest, BurnExpiredIntentsResponse};
use whisky::{
    calculate_tx_hash,
    data::{ByteString, PlutusData},
    Budget, WError, Wallet,
};

use crate::{
    config::AppConfig,
    scripts::{HydraUserIntentRedeemer, ScriptCache},
    utils::{hydra::get_hydra_tx_builder, proto::from_proto_utxos},
};

pub async fn handler(
    request: BurnExpiredIntentsRequest,
    app_owner_wallet: &Wallet,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<BurnExpiredIntentsResponse, WError> {
    let BurnExpiredIntentsRequest {
        intent_utxos,
        collateral_utxo,
        dex_order_book_utxo,
    } = request;

    let app_owner_vkey = &config.app_owner_vkey;

    let collateral = from_proto_utxos(&[collateral_utxo.unwrap()])[0].clone();
    let ref_input = from_proto_utxos(&[dex_order_book_utxo.unwrap()])[0].clone();
    let intents = from_proto_utxos(&intent_utxos);

    if intents.is_empty() {
        return Err(WError::new("handler", "No intent UTXOs provided"));
    }

    let mut tx_builder = get_hydra_tx_builder(true);
    let user_intent_spend = &scripts.user_intent_spend;
    let user_intent_mint = &scripts.user_intent_mint;

    log::info!(
        "[BURN_EXPIRED_INTENTS] Burning {} expired intents",
        intents.len()
    );

    // Spend each intent UTxO
    for intent_utxo in &intents {
        log::debug!(
            "[BURN_EXPIRED_INTENTS] Spending intent UTxO: {}#{}",
            intent_utxo.input.tx_hash,
            intent_utxo.input.output_index
        );

        tx_builder
            .spending_plutus_script_v3()
            .tx_in(
                &intent_utxo.input.tx_hash,
                intent_utxo.input.output_index,
                &intent_utxo.output.amount,
                &intent_utxo.output.address,
            )
            .tx_in_inline_datum_present()
            .tx_in_redeemer_value(&user_intent_spend.redeemer(ByteString::new(""), {
                Some(Budget {
                    mem: 7000000,
                    steps: 3000000000,
                })
            }))
            .spending_tx_in_reference(
                collateral.input.tx_hash.as_str(),
                user_intent_spend.ref_output_index,
                &user_intent_spend.hash,
                user_intent_spend.size,
            )
            .input_for_evaluation(&intent_utxo);
    }

    // Mint negative tokens to burn
    let burn_quantity = -(intents.len() as i128);
    tx_builder
        .mint_plutus_script_v3()
        .mint(burn_quantity, &user_intent_mint.hash, "")
        .mint_tx_in_reference(
            &collateral.input.tx_hash,
            user_intent_mint.ref_output_index,
            &user_intent_mint.hash,
            user_intent_mint.size,
        )
        .mint_redeemer_value(&user_intent_mint.redeemer(
            HydraUserIntentRedeemer::<PlutusData>::BurnIntent,
            Some(Budget {
                mem: 7000000,
                steps: 3000000000,
            }),
        ));

    let _ = tx_builder
        .input_for_evaluation(&user_intent_spend.ref_utxo(&collateral)?)
        .input_for_evaluation(&user_intent_mint.ref_utxo(&collateral)?)
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        .required_signer_hash(&app_owner_vkey)
        .tx_in_collateral(
            &collateral.input.tx_hash,
            collateral.input.output_index,
            &collateral.output.amount,
            &collateral.output.address,
        )
        .input_for_evaluation(&collateral)
        .change_address(&collateral.output.address)
        .complete_sync(None);

    let tx_hex = tx_builder.tx_hex();
    let tx_hash = calculate_tx_hash(&tx_hex)?;
    let signed_tx = app_owner_wallet.sign_tx(&tx_hex)?;

    log::info!("[BURN_EXPIRED_INTENTS] tx_hash: {}", tx_hash);

    Ok(BurnExpiredIntentsResponse { signed_tx, tx_hash })
}
