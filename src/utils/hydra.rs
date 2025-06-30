use whisky::{OfflineTxEvaluator, TxBuilder, TxBuilderParam};

use crate::config::hydra::get_hydra_pp;

pub fn get_hydra_tx_builder() -> TxBuilder {
    let tx_builder = TxBuilder::new(TxBuilderParam {
        evaluator: Some(Box::new(OfflineTxEvaluator::new())),
        fetcher: None,
        submitter: None,
        params: Some(get_hydra_pp()),
    });
    tx_builder
}
