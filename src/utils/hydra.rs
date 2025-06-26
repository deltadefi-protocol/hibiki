use whisky::{OfflineTxEvaluator, TxBuilder};

use crate::config::hydra::get_hydra_pp;

pub fn get_hydra_tx_builder() -> TxBuilder {
    let mut tx_builder = TxBuilder::new_core();
    tx_builder.evaluator = Some(Box::new(OfflineTxEvaluator::new()));
    tx_builder.protocol_params = Some(get_hydra_pp());
    tx_builder
}
