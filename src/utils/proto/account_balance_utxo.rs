use hibiki_proto::services::BalanceUtxos;
use whisky::{Asset, UTxO};

use crate::utils::proto::{from_proto_amount, from_proto_utxo};

pub fn from_proto_balance_utxos(proto: &BalanceUtxos) -> (Vec<Asset>, Vec<UTxO>) {
    let utxos: Vec<UTxO> = proto.utxos.iter().map(|u| from_proto_utxo(u)).collect();
    let assets = from_proto_amount(&proto.updated_balance_l1);
    (assets, utxos)
}
