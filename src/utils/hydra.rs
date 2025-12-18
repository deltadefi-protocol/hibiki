use whisky::{
    blockfrost::utils::{normalize_plutus_script, to_script_ref, ScriptType},
    csl::{self, PlutusScript, ScriptRef},
    OfflineTxEvaluator, TxBuilder, TxBuilderParam, WError,
};

use crate::config::hydra::get_hydra_pp;

pub fn get_script_ref_hex(cbor: &str) -> Result<String, WError> {
    let normalized =
        normalize_plutus_script(cbor).map_err(WError::from_err("normalize_plutus_script"))?;
    let script: PlutusScript =
        PlutusScript::from_hex_with_version(&normalized, &csl::Language::new_plutus_v3())
            .map_err(WError::from_err("from_hex_with_version"))?;
    let script_ref: ScriptRef = to_script_ref(&ScriptType::Plutus(script));
    Ok(hex::encode(script_ref.to_unwrapped_bytes()))
}

pub fn get_hydra_tx_builder() -> TxBuilder {
    let mut tx_builder = TxBuilder::new(TxBuilderParam {
        evaluator: Some(Box::new(OfflineTxEvaluator::new())),
        fetcher: None,
        submitter: None,
        params: Some(get_hydra_pp()),
    });
    tx_builder.serializer.tx_evaluation_multiplier_percentage = 150;
    tx_builder
}
