use hibiki_proto::services::{InternalTransferRequest, InternalTransferResponse};
use whisky::{Budget, WData, WError, WRedeemer};

use crate::{scripts::hydra_user_intent_minting_blueprint, utils::hydra::get_hydra_tx_builder};

// Changed from async to synchronous function to avoid Send issues with Rc<T>
pub fn handler(request: InternalTransferRequest) -> Result<InternalTransferResponse, WError> {
    let mut tx_builder = get_hydra_tx_builder();
    let user_intent_mint = hydra_user_intent_minting_blueprint();

    // Use the blocking variant of complete() since we're now in a synchronous context
    tx_builder
        .mint_plutus_script_v3()
        .mint(1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&WRedeemer {
            data: WData::JSON("".to_string()),
            ex_units: Budget::default(),
        })
        .minting_script(&user_intent_mint.cbor)
        .complete_sync(None)?;

    let tx_hex = tx_builder.tx_hex();

    Ok(InternalTransferResponse {
        tx_hex: tx_hex,
        dex_net_deposit_utxo_tx_index: 0,
        updated_merkle_root: "".to_string(),
    })
}
