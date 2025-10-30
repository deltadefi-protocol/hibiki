use whisky::{calculate_tx_hash, CSLParser, WError, Wallet};

use crate::services::{SignTransactionRequest, SignTransactionResponse};

pub fn check_signature_sign_tx(wallet: &Wallet, tx_hex: &str) -> Result<String, WError> {
    let signed_tx = wallet.sign_tx(tx_hex).unwrap();

    let mut tx_parser = CSLParser::new();
    let is_transaction_fully_signed =
        tx_parser
            .check_all_required_signers(tx_hex)
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

pub fn handler(
    request: SignTransactionRequest,
    app_owner_wallet: &Wallet,
) -> Result<SignTransactionResponse, WError> {
    let tx_hex = request.tx_hex;
    let signed_tx = check_signature_sign_tx(&app_owner_wallet, &tx_hex)?;
    let tx_hash = calculate_tx_hash(&signed_tx)?;
    let reply = SignTransactionResponse { signed_tx, tx_hash };
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use crate::utils::wallet::get_app_owner_wallet;

    use super::*;
    use dotenv::dotenv;

    #[test]
    fn test_app_sign_tx() {
        dotenv().ok();
        let app_owner_wallet = get_app_owner_wallet();
        let tx_hex = "84a800d9010281825820e38178967c200c81b4d7052e81de78a32c22a1ca26c1737f04643d5ee237ad9419020a0182a300581d70eb0a5938244e92fd172560f530bf959724b10353a26f276ea8bbb3cc018200a1581c463e70d04718e253757523698184cb7090b0430e89dc025c4c8e392ca14001028201d81858c7d87c9fd8799fd8799f50fb73c3bd256949a480c47abaad3dfa4fd8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581c71052cf0c2562d16eedd20e6e22e5b82173630f9a45ff8d42c38e29fffffffd8799fd8799f50e0e1622f99434f2c867afc3dcb6732b3d8799f581cfdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4ffd8799f581c229d96e64aa5878fc93ba2ee9081126052d62974da032f1e5998be5dffffffa140a1401a000f4240ff82583900fa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c89a2f36d3033bf4be236847143916e2e237de49069844934ac88f4e500020009a1581c463e70d04718e253757523698184cb7090b0430e89dc025c4c8e392ca140010b58208ba3b26901576dfc1757835eca10292d9d0324e3779e9b15b908e4f7459edcb90dd9010281825820e38178967c200c81b4d7052e81de78a32c22a1ca26c1737f04643d5ee237ad941903e80ed9010282581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b6612d9010281825820ace128c7ab85836aed1f4f188df6a85e6b103d21518af570fa81deaef6018ff400a207d901028158b558b30101009800aba2a6011e581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c00a6010746382d6d696e740048c8c8c8c88c88966002646464646464660020026eb0c038c03cc03cc03cc03cc03cc03cc03cc03cc030dd5180718061baa0072259800800c52844c96600266e3cdd71808001005c528c4cc00c00c00500d1808000a01c300c300d002300b001300b002300900130063754003149a26cac8028dd7000ab9a5573caae7d5d0905a182010082d87f9fd8799fd8799f50fb73c3bd256949a480c47abaad3dfa4fd8799f581c04845038ee499ee8bc0afe56f688f27b2dd76f230d3698a9afcc1b66ffd8799f581c71052cf0c2562d16eedd20e6e22e5b82173630f9a45ff8d42c38e29fffffffd8799fd8799f50e0e1622f99434f2c867afc3dcb6732b3d8799f581cfdeb4bf0e8c077114a4553f1e05395e9fb7114db177f02f7b65c8de4ffd8799f581c229d96e64aa5878fc93ba2ee9081126052d62974da032f1e5998be5dffffffa140a1401a000f4240ff820000f5f6".to_string();
        let signed_tx = check_signature_sign_tx(&app_owner_wallet, &tx_hex).unwrap();
        assert!(!signed_tx.is_empty());
    }
}
