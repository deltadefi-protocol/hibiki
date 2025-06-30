use hibiki_proto::services::{ProcessTransferRequest, ProcessTransferResponse};
use whisky::{
    calculate_tx_hash,
    data::{Constr0, Value},
    Budget, WError, WRedeemer,
};

use crate::{
    config::AppConfig,
    handler::sign_transaction,
    scripts::{
        hydra_account_balance_spending_blueprint, hydra_user_intent_minting_blueprint,
        hydra_user_intent_spending_blueprint, HydraAccountBalanceDatum, HydraUserIntentRedeemer,
        UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{from_proto_balance_utxo, from_proto_utxo},
    },
};

pub async fn handler(request: ProcessTransferRequest) -> Result<ProcessTransferResponse, WError> {
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
            data: user_intent_spend.redeemer(Constr0::new(())),
            ex_units: Budget::default(),
        })
        .tx_in_script(&user_intent_spend.cbor)
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
            data: user_intent_spend.redeemer(Constr0::new(())),
            ex_units: Budget::default(),
        })
        .tx_in_script(&user_intent_spend.cbor)
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
        // common
        .tx_in_collateral(
            &colleteral.input.tx_hash,
            colleteral.input.output_index,
            &colleteral.output.amount,
            &colleteral.output.address,
        )
        .change_address(&request.address)
        .required_signer_hash(&app_owner_vkey)
        .complete(None)
        .await?;

    let tx_hex = tx_builder.tx_hex();
    let tx_hash = calculate_tx_hash(&tx_hex)?;
    let signed_tx = sign_transaction::app_sign_tx(&tx_hex)?;

    Ok(ProcessTransferResponse {
        signed_tx,
        tx_hash,
        account_utxo_tx_index: 0,
        receiver_account_utxo_tx_index: 1,
    })
}
