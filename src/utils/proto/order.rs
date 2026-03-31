use whisky::{Asset, UTxO, WError};

use crate::utils::proto::{from_proto_amount, from_proto_utxo};

pub struct OrderInput {
    pub order_id: String,
    pub order_utxo: UTxO,
    pub updated_order_size: u64,
    pub updated_price_times_one_tri: u64,
    pub updated_order_value_l1: Vec<Asset>,
}

pub fn from_proto_order(proto_order: &hibiki_proto::services::Order) -> Result<OrderInput, WError> {
    Ok(OrderInput {
        order_id: proto_order.order_id.replace("-", ""),
        order_utxo: from_proto_utxo(
            proto_order
                .order_utxo
                .as_ref()
                .ok_or_else(WError::from_opt("from_proto_order", "order_utxo"))?,
        ),
        updated_order_size: proto_order.updated_order_size,
        updated_price_times_one_tri: proto_order.updated_price_times_one_tri,
        updated_order_value_l1: from_proto_amount(&proto_order.updated_order_value_l1),
    })
}
