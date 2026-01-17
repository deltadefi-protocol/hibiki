use hibiki_proto::services::{IntentTxResponse, PlaceOrderRequest};
use whisky::WError;

pub async fn handler(_request: PlaceOrderRequest) -> Result<IntentTxResponse, WError> {
    Err(WError::new("Not implemented", "gm"))
}
