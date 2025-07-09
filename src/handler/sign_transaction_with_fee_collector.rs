use hibiki_proto::services::{SignTransactionRequest, SignTransactionResponse};
use whisky::{calculate_tx_hash, WError};

use crate::{
    handler::sign_transaction::check_signature_sign_tx, utils::wallet::get_fee_collector_wallet,
};

pub fn fee_collector_sign_tx(tx_hex: &str) -> Result<String, WError> {
    let fee_collector_owner_wallet = get_fee_collector_wallet();
    check_signature_sign_tx(&fee_collector_owner_wallet, tx_hex)
}

pub fn handler(request: SignTransactionRequest) -> Result<SignTransactionResponse, WError> {
    let tx_hex = request.tx_hex;

    let signed_tx = fee_collector_sign_tx(&tx_hex)?;
    let tx_hash = calculate_tx_hash(&signed_tx)?;
    let reply = SignTransactionResponse { signed_tx, tx_hash };
    Ok(reply)
}
