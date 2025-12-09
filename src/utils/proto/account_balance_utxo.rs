use hibiki_proto::services::{BalanceUtxo, BalanceUtxos};
use whisky::{Asset, UTxO};

use crate::utils::proto::{from_proto_amount, from_proto_utxo};

pub fn from_proto_balance_utxo(proto: &BalanceUtxo) -> (Vec<Asset>, UTxO) {
    let utxo = from_proto_utxo(proto.utxo.as_ref().unwrap());
    let assets = from_proto_amount(&proto.updated_balance);
    (assets, utxo)
}

pub fn from_proto_balance_utxos(proto: &BalanceUtxos) -> (Vec<Asset>, Vec<UTxO>) {
    let utxos: Vec<UTxO> = proto
        .utxo
        .iter()
        .map(|u| from_proto_utxo(u))
        .collect();
    let assets = from_proto_amount(&proto.updated_balance);
    (assets, utxos)
}
