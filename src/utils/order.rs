use hibiki_proto::services::OrderType as ProtoOrderType;
use whisky::data::{ByteString, Tuple};

use crate::scripts::{Order, OrderType, UserAccount};

pub fn to_order_datum(
    order_id: &str,
    base_token_unit: &str,
    quote_token_unit: &str,
    is_buy: bool,
    list_price_times_one_tri: u64,
    order_size: u64,
    commission_rate_bp: u64,
    user_account: &UserAccount,
    order_type: ProtoOrderType,
) -> Order {
    let split_token_unit = |unit: &str| -> Tuple<(ByteString, ByteString)> {
        let (policy_id, asset_name) = unit.split_at(56);
        Tuple::new((ByteString::new(policy_id), ByteString::new(asset_name)))
    };
    let base_tuple = split_token_unit(&base_token_unit);
    let quote_tuple = split_token_unit(&quote_token_unit);
    let datum_order_type: OrderType = match order_type {
        ProtoOrderType::Limit => OrderType::LimitOrder,
        ProtoOrderType::Market => OrderType::MarketOrder,
    };

    Order::from(
        &order_id.replace("-", ""),
        base_tuple,
        quote_tuple,
        is_buy,
        list_price_times_one_tri.into(),
        order_size.into(),
        commission_rate_bp.into(),
        user_account.clone(),
        datum_order_type,
    )
}
