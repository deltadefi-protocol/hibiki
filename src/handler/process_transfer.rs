use hibiki_proto::services::{ProcessTransferRequest, ProcessTransferResponse};
use whisky::{Budget, WData, WError, WRedeemer};

use crate::{scripts::hydra_user_intent_minting_blueprint, utils::hydra::get_hydra_tx_builder};

pub fn handler(request: ProcessTransferRequest) -> Result<ProcessTransferResponse, WError> {
    // let mut tx_builder = get_hydra_tx_builder();
    // let user_intent_mint = hydra_user_intent_minting_blueprint();

    // tx_builder
    //     .mint_plutus_script_v3()
    //     .mint(1, &user_intent_mint.hash, "")
    //     .mint_redeemer_value(&WRedeemer {
    //         data: WData::JSON("".to_string()),
    //         ex_units: Budget::default(),
    //     })
    //     .minting_script(&user_intent_mint.cbor)
    //     .complete(None)
    //     .await?;

    // let tx_hex = tx_builder.tx_hex();

    Ok(ProcessTransferResponse {
        tx_hash: todo!(),
        dex_net_deposit_utxo_tx_index: todo!(),
        updated_merkle_root: todo!(),
    })
}
