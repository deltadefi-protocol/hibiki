use whisky::{data::PlutusDataJson, WData, WError};

use crate::{
    scripts::{MasterIntent, TransferIntent, UserTradeAccount},
    services::{SerializeDatumResponse, SerializeTransferalIntentDatumRequest},
    utils::{
        proto::{assets_to_mvalue, from_proto_amount},
        token::to_hydra_token,
    },
};

pub fn handler(
    request: SerializeTransferalIntentDatumRequest,
) -> Result<SerializeDatumResponse, WError> {
    let from_account = UserTradeAccount::from_proto(&request.account.unwrap());
    let to_account = UserTradeAccount::from_proto(&request.receiver_account.unwrap());
    let transfer_amount =
        assets_to_mvalue(&to_hydra_token(&from_proto_amount(&request.to_transfer)));

    let hydra_account_intent = TransferIntent::new(to_account, transfer_amount);
    let datum_json = MasterIntent::new(from_account, hydra_account_intent);

    let datum = WData::JSON(datum_json.to_json_string());
    let reply = SerializeDatumResponse {
        plutus_data: datum.to_cbor()?,
        data_hash: datum.to_hash()?,
    };
    Ok(reply)
}
