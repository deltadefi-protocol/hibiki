use hibiki_proto::services::{SignTransactionRequest, SignTransactionResponse};
use whisky::{calculate_tx_hash, WError, Wallet};

use super::sign_transaction::check_signature_sign_tx;

pub fn handler(
    request: SignTransactionRequest,
    fee_collector_owner_wallet: &Wallet,
) -> Result<SignTransactionResponse, WError> {
    let tx_hex = request.tx_hex;

    let signed_tx = check_signature_sign_tx(fee_collector_owner_wallet, &tx_hex)?;
    let tx_hash = calculate_tx_hash(&signed_tx)?;
    let reply = SignTransactionResponse { signed_tx, tx_hash };
    Ok(reply)
}
