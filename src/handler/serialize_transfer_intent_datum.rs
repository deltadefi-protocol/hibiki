use whisky::{
    data::{PlutusDataToJson, Value},
    WData, WError,
};

use crate::{
    scripts::{HydraUserIntentDatum, UserAccount},
    services::{SerializeDatumResponse, SerializeTransferalIntentDatumRequest},
    utils::proto::from_proto_amount,
};

pub fn handler(
    request: SerializeTransferalIntentDatumRequest,
) -> Result<SerializeDatumResponse, WError> {
    let datum_json = HydraUserIntentDatum::TransferIntent(
        UserAccount::from_proto(&request.account.unwrap()),
        UserAccount::from_proto(&request.receiver_account.unwrap()),
        Value::from_asset_vec(&from_proto_amount(&request.to_transfer)),
    );
    let datum = WData::JSON(datum_json.to_json_string());
    let reply = SerializeDatumResponse {
        plutus_data: datum.to_cbor()?,
        data_hash: datum.to_hash()?,
    };
    Ok(reply)
}
