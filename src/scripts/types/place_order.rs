use whisky::data::{Bool, ByteArray, ByteString, Constr0, Int, PlutusDataJson, Tuple};

use crate::scripts::bar::{OrderDetails, OrderType, PlaceOrderIntent};
use crate::scripts::MValue;

/// Order type from proto enum string
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoOrderType {
    Limit,
    Market,
}

impl ProtoOrderType {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "LIMIT" | "ORDER_TYPE_LIMIT" => ProtoOrderType::Limit,
            "MARKET" | "ORDER_TYPE_MARKET" => ProtoOrderType::Market,
            _ => ProtoOrderType::Limit, // Default to Limit
        }
    }

    pub fn to_order_type(&self) -> OrderType {
        match self {
            ProtoOrderType::Limit => OrderType::LimitOrder,
            ProtoOrderType::Market => OrderType::MarketOrder,
        }
    }
}

impl OrderDetails {
    /// Create new order details
    ///
    /// # Arguments
    /// * `order_id` - Unique order identifier (UUID without dashes as hex)
    /// * `base_token_l2` - Base token unit in L2 format (policy_id + asset_name)
    /// * `quote_token_l2` - Quote token unit in L2 format
    /// * `is_buy` - True if buy order, false if sell
    /// * `price` - Price * 10^12 (one trillion for precision)
    /// * `size` - Order size in base token units
    /// * `commission_bp` - Commission rate in basis points
    /// * `account` - User account placing the order
    /// * `order_type` - Limit or Market order
    pub fn new(
        order_id: &str,
        base_token_l2: &str,
        quote_token_l2: &str,
        is_buy: bool,
        price: i64,
        size: i64,
        commission_bp: i64,
        account: UserAccount,
        order_type: OrderType,
    ) -> Self {
        let order_id_bytes = ByteArray::new(&order_id.replace("-", ""));

        // Parse base token: first 56 chars is policy_id, rest is asset_name
        let base_sanitized = sanitize_unit(base_token_l2);
        let base_token_tuple = create_token_tuple(&base_sanitized);

        // Parse quote token
        let quote_sanitized = sanitize_unit(quote_token_l2);
        let quote_token_tuple = create_token_tuple(&quote_sanitized);

        OrderDetails(Constr0::new(Box::new((
            order_id_bytes,
            base_token_tuple,
            quote_token_tuple,
            Bool::new(is_buy),
            Int::new(price as i128),
            Int::new(size as i128),
            Int::new(commission_bp as i128),
            account,
            order_type,
        ))))
    }
}

impl PlaceOrderIntent {
    /// Create a new place order intent
    ///
    /// # Arguments
    /// * `order_details` - The order details
    /// * `authorized_value` - The authorized account value for this order (in L2 format)
    pub fn new(order_details: OrderDetails, authorized_value: MValue) -> Self {
        PlaceOrderIntent(Constr0::new(Box::new((order_details, authorized_value))))
    }
}

use crate::scripts::bar::UserAccount;

/// Create a trade intent datum JSON string for placing an order
/// This creates a ConStr0([account, place_order_intent])
pub fn create_place_order_trade_intent_datum(
    account: &UserAccount,
    intent: &PlaceOrderIntent,
) -> String {
    // TradeIntent is ConStr0([account, intent])
    format!(
        r#"{{"constructor":0,"fields":[{},{}]}}"#,
        account.to_json_string(),
        intent.to_json_string()
    )
}

/// Create a mint trade intent redeemer JSON string for placing an order
/// This creates a ConStr0([account, place_order_intent]) - same structure as datum
pub fn create_place_order_mint_redeemer(
    account: &UserAccount,
    intent: &PlaceOrderIntent,
) -> String {
    // MintTradeIntent is ConStr0([account, intent])
    format!(
        r#"{{"constructor":0,"fields":[{},{}]}}"#,
        account.to_json_string(),
        intent.to_json_string()
    )
}

/// Sanitize token unit - ensure it's a valid hex string
fn sanitize_unit(unit: &str) -> String {
    // Remove any non-hex characters and ensure proper length
    let clean: String = unit.chars().filter(|c| c.is_ascii_hexdigit()).collect();

    // If it's lovelace (empty or "lovelace"), return empty policy
    if clean.is_empty() || unit.to_lowercase() == "lovelace" {
        return String::new();
    }

    clean
}

/// Create a token tuple (PolicyId, AssetName) from a sanitized unit string
fn create_token_tuple(unit: &str) -> Tuple<(ByteString, ByteString)> {
    if unit.is_empty() {
        // Lovelace case - empty policy_id and asset_name
        Tuple::new((ByteString::new(""), ByteString::new("")))
    } else if unit.len() <= 56 {
        // Only policy_id, no asset_name
        Tuple::new((ByteString::new(unit), ByteString::new("")))
    } else {
        // policy_id + asset_name
        let policy_id = &unit[..56];
        let asset_name = &unit[56..];
        Tuple::new((ByteString::new(policy_id), ByteString::new(asset_name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_order_type() {
        assert_eq!(ProtoOrderType::from_str("LIMIT"), ProtoOrderType::Limit);
        assert_eq!(
            ProtoOrderType::from_str("ORDER_TYPE_LIMIT"),
            ProtoOrderType::Limit
        );
        assert_eq!(ProtoOrderType::from_str("MARKET"), ProtoOrderType::Market);
        assert_eq!(
            ProtoOrderType::from_str("ORDER_TYPE_MARKET"),
            ProtoOrderType::Market
        );
    }

    #[test]
    fn test_sanitize_unit() {
        assert_eq!(sanitize_unit("lovelace"), "");
        assert_eq!(sanitize_unit(""), "");
        assert_eq!(sanitize_unit("abc123"), "abc123");
    }
}
