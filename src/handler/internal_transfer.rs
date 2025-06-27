use hibiki_proto::services::{IntentTxResponse, InternalTransferRequest};
use whisky::{calculate_tx_hash, data::Value, Asset, Budget, WError, WRedeemer};

use crate::{
    config::AppConfig,
    scripts::{
        hydra_user_intent_minting_blueprint, hydra_user_intent_spending_blueprint,
        HydraUserIntentDatum, HydraUserIntentRedeemer, UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{from_proto_amount, from_proto_utxo},
    },
};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub fn handler(request: InternalTransferRequest) -> Result<IntentTxResponse, WError> {
    let AppConfig { app_owner_vkey, .. } = AppConfig::new();
    let colleteral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let empty_utxo = from_proto_utxo(request.empty_utxo.as_ref().unwrap());

    let mut tx_builder = get_hydra_tx_builder();
    let user_intent_mint = hydra_user_intent_minting_blueprint();
    let user_intent_spend = hydra_user_intent_spending_blueprint();

    let from_account = UserAccount::from_proto(request.account.unwrap());
    let to_account = UserAccount::from_proto(request.receiver_account.unwrap());
    let transfer_amount = Value::from_asset_vec(&from_proto_amount(&request.to_transfer));

    let redeemer_json = HydraUserIntentRedeemer::MintTransferIntent(
        from_account.clone(),
        to_account.clone(),
        transfer_amount.clone(),
    );

    let datum_json =
        HydraUserIntentDatum::TransferIntent(from_account, to_account, transfer_amount);

    // Use the blocking variant of complete() since we're now in a synchronous context
    tx_builder
        .mint_plutus_script_v3()
        .mint(1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&WRedeemer {
            data: user_intent_mint.redeemer(redeemer_json),
            ex_units: Budget::default(),
        })
        .minting_script(&user_intent_mint.cbor)
        .tx_out(
            &user_intent_spend.address,
            &[Asset::new_from_str(&user_intent_mint.hash, "1")],
        )
        .tx_out_inline_datum_value(&user_intent_spend.datum(datum_json))
        .tx_in(
            &empty_utxo.input.tx_hash,
            empty_utxo.input.output_index,
            &empty_utxo.output.amount,
            &empty_utxo.output.address,
        )
        .tx_out(&empty_utxo.output.address, &empty_utxo.output.amount)
        .required_signer_hash(&app_owner_vkey)
        .tx_in_collateral(
            &colleteral.input.tx_hash,
            colleteral.input.output_index,
            &colleteral.output.amount,
            &colleteral.output.address,
        )
        .change_address(&request.address)
        .complete_sync(None)?;

    let tx_hex = tx_builder.tx_hex();
    let tx_hash = calculate_tx_hash(&tx_hex)?;

    Ok(IntentTxResponse {
        tx_hex: tx_hex,
        tx_hash: tx_hash,
        tx_index: 0,
        new_empty_utxo_tx_index: 1,
    })
}
