use hibiki_proto::services::{CreateHydraAccountUtxoRequest, CreateHydraAccountUtxoResponse};
use whisky::{calculate_tx_hash, data::Value, Asset, Budget, WError, WRedeemer};

use crate::{
    config::AppConfig,
    handler::sign_transaction,
    scripts::{
        hydra_account_balance_minting_blueprint, hydra_account_balance_spending_blueprint,
        HydraAccountBalanceDatum, MintPolarity, UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{from_proto_txin, from_proto_utxo},
    },
};

pub fn handler(
    request: CreateHydraAccountUtxoRequest,
) -> Result<CreateHydraAccountUtxoResponse, WError> {
    let AppConfig { app_owner_vkey, .. } = AppConfig::new();

    let colleteral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let ref_input = from_proto_txin(request.dex_order_book_input.as_ref().unwrap());
    let empty_utxo = from_proto_utxo(request.empty_utxo.as_ref().unwrap());
    let account = UserAccount::from_proto(request.account.as_ref().unwrap());

    let account_balance_spend = hydra_account_balance_spending_blueprint();
    let account_balance_mint = hydra_account_balance_minting_blueprint();

    let mut tx_builder = get_hydra_tx_builder();
    tx_builder
        // reference oracle utxo
        .read_only_tx_in_reference(&ref_input.tx_hash, ref_input.output_index, None)
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
        .tx_out(&empty_utxo.output.address, &empty_utxo.output.amount)
        // common
        .tx_in_collateral(
            &colleteral.input.tx_hash,
            colleteral.input.output_index,
            &colleteral.output.amount,
            &colleteral.output.address,
        )
        .change_address(&request.address)
        .required_signer_hash(&app_owner_vkey)
        .complete_sync(None)?;

    let tx_hex = tx_builder.tx_hex();
    let tx_hash = calculate_tx_hash(&tx_hex)?;
    let signed_tx = sign_transaction::app_sign_tx(&tx_hex)?;

    Ok(CreateHydraAccountUtxoResponse {
        signed_tx,
        tx_hash,
        account_utxo_tx_index: 0,
        new_empty_utxo_tx_index: 1,
    })
}
