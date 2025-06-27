use hibiki_proto::services::BalanceUtxo;
use whisky::{Asset, UTxO};

use crate::utils::proto::{from_proto_amount, from_proto_utxo};

pub fn from_proto_balance_utxo(proto: &BalanceUtxo) -> (Vec<Asset>, UTxO) {
    let utxo = from_proto_utxo(proto.utxo.as_ref().unwrap());
    let assets = from_proto_amount(&proto.updated_balance);
    (assets, utxo)
}
