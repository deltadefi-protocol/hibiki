use whisky::data::Int;

use crate::scripts::{Order, OrderType};

impl Order {
    // Returns a new Order with updated price and size
    // Note: does not modify self, but returns a new instance
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
}
