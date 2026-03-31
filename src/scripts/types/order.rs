use whisky::{data::Int, PlutusDataCbor, WError};

use crate::scripts::{Order, OrderType, UserAccount};

impl Order {
    /// Returns a new Order with updated price and size
    /// Note: does not modify self, but returns a new instance
    pub fn update_order(
        &self,
        order_size: u64,
        price_times_one_tri: u64,
        order_type: OrderType,
    ) -> Order {
        let mut old_order = self.0.clone();
        old_order.fields.4 = Int::new(price_times_one_tri.into());
        old_order.fields.5 = Int::new(order_size.into());
        old_order.fields.8 = order_type;
        Order(old_order)
    }

    /// Extract account_id (hex without hyphens) and user_account from order datum CBOR
    pub fn parse_user_account(plutus_data_cbor: &str) -> Result<(String, UserAccount), WError> {
        let order_datum = Order::from_cbor(plutus_data_cbor)?;

        // fields.7 is UserAccount
        let user_account = order_datum.0.fields.7.clone();

        let account_id = match &user_account {
            UserAccount::UserTradeAccount(boxed) => {
                let (account, _) = boxed.as_ref();
                account.0.fields.0.bytes.clone()
            }
            UserAccount::UserFundingAccount(boxed) => {
                let (account, _) = boxed.as_ref();
                account.0.fields.0.bytes.clone()
            }
            UserAccount::UserMobileAccount(boxed) => {
                let (account, _) = boxed.as_ref();
                account.0.fields.0.bytes.clone()
            }
        };

        Ok((account_id, user_account))
    }
}
