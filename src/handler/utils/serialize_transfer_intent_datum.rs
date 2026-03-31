use whisky::{data::PlutusDataJson, WData, WError};

use crate::{
    scripts::{HydraAccountIntent, HydraUserIntentDatum, ScriptCache, UserAccount},
    services::{SerializeDatumResponse, SerializeTransferalIntentDatumRequest},
    utils::{
        proto::{assets_to_mvalue, from_proto_amount},
        token::to_hydra_token,
    },
};

pub fn handler(
    request: SerializeTransferalIntentDatumRequest,
    scripts: &ScriptCache,
) -> Result<SerializeDatumResponse, WError> {
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let from_account =
        UserAccount::from_proto_trade_account(&request.account.unwrap(), account_ops_script_hash);
    let to_account = UserAccount::from_proto_trade_account(
        &request.receiver_account.unwrap(),
        account_ops_script_hash,
    );
    let transfer_amount =
        assets_to_mvalue(&to_hydra_token(&from_proto_amount(&request.to_transfer)));

    let hydra_account_intent =
        HydraAccountIntent::TransferIntent(Box::new((to_account, transfer_amount)));
    let datum_json =
        HydraUserIntentDatum::MasterIntent(Box::new((from_account, hydra_account_intent)));

    let datum = WData::JSON(datum_json.to_json_string());
    let reply = SerializeDatumResponse {
        plutus_data: datum.to_cbor()?,
        data_hash: datum.to_hash()?,
    };
    Ok(reply)
}
