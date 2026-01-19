use whisky::{
    data::{PlutusDataJson, Value},
    Asset, PlutusDataCbor, WError,
};

use crate::scripts::{HydraAccountIntent, HydraOrderBookIntent, HydraUserIntentDatum, Order};

impl<T: PlutusDataCbor> HydraUserIntentDatum<T> {
    pub fn get_master_intent(&self) -> Result<&T, WError> {
        let (_, intent_datum) = match self {
            HydraUserIntentDatum::MasterIntent(boxed) => boxed.as_ref(),
            _ => {
                return Err(WError::new(
                    "get_master_intent",
                    "Expected MasterIntent variant",
                ))
            }
        };
        Ok(intent_datum)
    }

    pub fn get_trade_intent(&self) -> Result<&T, WError> {
        let (_, intent_datum) = match self {
            HydraUserIntentDatum::TradeIntent(boxed) => boxed.as_ref(),
            _ => {
                return Err(WError::new(
                    "get_trade_intent",
                    "expected TradeIntent variant",
                ))
            }
        };
        Ok(intent_datum)
    }
}

impl HydraUserIntentDatum<HydraAccountIntent> {
    pub fn get_transfer_amount(&self) -> Result<Vec<Asset>, WError> {
        let intent_datum = self.get_master_intent()?;
        let value = match intent_datum {
            HydraAccountIntent::TransferIntent(boxed) => {
                let (_, transfer_amount) = boxed.as_ref();
                Value::from_json(&transfer_amount.to_json()).unwrap()
            }
            _ => {
                return Err(WError::new(
                    "get_transfer_amount - intent_datum",
                    "Expected TransferIntent variant",
                ))
            }
        };

        Ok(value.to_asset_vec())
    }
}

impl HydraUserIntentDatum<HydraOrderBookIntent> {
    pub fn get_order(&self) -> Result<&Order, WError> {
        let intent_datum = self.get_trade_intent()?;
        let (order, _) = match intent_datum {
            HydraOrderBookIntent::PlaceOrderIntent(boxed) => boxed.as_ref(),
            _ => {
                return Err(WError::new(
                    "get_order - intent_datum",
                    "expected PlaceOrderIntent datum",
                ))
            }
        };
        Ok(order)
    }
}
