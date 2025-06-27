use whisky::{calculate_tx_hash, CSLParser, WError};

use crate::{
    services::{SignTransactionRequest, SignTransactionResponse},
    utils::wallet::get_app_owner_wallet,
};

pub fn app_sign_tx(tx_hex: &str) -> Result<String, WError> {
    let app_owner_wallet = get_app_owner_wallet();
    let signed_tx = app_owner_wallet.sign_tx(&tx_hex).unwrap();

    let mut tx_parser = CSLParser::new();
    let is_transaction_fully_signed =
        tx_parser
            .check_all_required_signers(&tx_hex)
            .map_err(WError::from_err(
                "SignTransaction - check_all_required_signers",
            ))?;

    if !is_transaction_fully_signed {
        return Err(WError::new(
            "SignTransaction - check_all_required_signers",
            "Transaction is not fully signed",
        ));
    }
    Ok(signed_tx)
}

pub fn handler(request: SignTransactionRequest) -> Result<SignTransactionResponse, WError> {
    let tx_hex = request.tx_hex;
    let signed_tx = app_sign_tx(&tx_hex)?;
    let tx_hash = calculate_tx_hash(&signed_tx)?;
    let reply = SignTransactionResponse { signed_tx, tx_hash };
    Ok(reply)
}
