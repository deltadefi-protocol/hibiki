use whisky::{OfflineTxEvaluator, TxBuilder, TxBuilderParam, UTxO, UtxoInput, UtxoOutput};

use crate::{
    config::hydra::get_hydra_pp,
    constant::L2_REF_SCRIPTS_INDEX,
    scripts::{
        hydra_account_balance_minting_blueprint, hydra_account_balance_spending_blueprint,
        hydra_internal_transfer_blueprint, hydra_user_intent_minting_blueprint,
        hydra_user_intent_spending_blueprint,
    },
};

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

/// Create a reference UTXO for a script at the given index with script_ref (CBOR) and script_hash
///
/// # Arguments
/// * `tx_hash` - Transaction hash containing the reference script
/// * `output_index` - Output index in the transaction (should match script index from L2_REF_SCRIPTS_INDEX)
/// * `address` - Address where the script UTXO is located
/// * `script_ref` - The CBOR-encoded script
/// * `script_hash` - The hash of the script
///
/// # Returns
/// A UTxO with script_ref and script_hash, but empty amount, data_hash, and plutus_data
fn create_ref_script_utxo(
    tx_hash: String,
    output_index: u32,
    address: String,
    script_ref: String,
    script_hash: String,
) -> UTxO {
    UTxO {
        input: UtxoInput {
            tx_hash,
            output_index,
        },
        output: UtxoOutput {
            address,
            amount: vec![],
            data_hash: None,
            plutus_data: None,
            script_ref: Some(script_ref),
            script_hash: Some(script_hash),
        },
    }
}

/// Get reference script UTxOs based on the L2 script index map
/// Only includes scripts that are actually implemented in the Rust codebase
pub mod ref_scripts {
    use super::*;

    /// Hydra internal transfer withdrawal script
    pub fn hydra_internal_transfer(tx_hash: String, address: String) -> UTxO {
        let blueprint = hydra_internal_transfer_blueprint();
        create_ref_script_utxo(
            tx_hash,
            L2_REF_SCRIPTS_INDEX
                .account_operation
                .hydra_internal_transfer,
            address,
            blueprint.cbor,
            blueprint.hash,
        )
    }

    /// Hydra user intent minting script
    pub fn hydra_user_intent_mint(tx_hash: String, address: String) -> UTxO {
        let blueprint = hydra_user_intent_minting_blueprint();
        create_ref_script_utxo(
            tx_hash,
            L2_REF_SCRIPTS_INDEX.hydra_user_intent.mint,
            address,
            blueprint.cbor,
            blueprint.hash,
        )
    }

    /// Hydra user intent spending script
    pub fn hydra_user_intent_spend(tx_hash: String, address: String) -> UTxO {
        let blueprint = hydra_user_intent_spending_blueprint();
        create_ref_script_utxo(
            tx_hash,
            L2_REF_SCRIPTS_INDEX.hydra_user_intent.spend,
            address,
            blueprint.cbor,
            blueprint.hash,
        )
    }

    /// Hydra account balance minting script
    pub fn hydra_account_balance_mint(tx_hash: String, address: String) -> UTxO {
        let blueprint = hydra_account_balance_minting_blueprint();
        create_ref_script_utxo(
            tx_hash,
            L2_REF_SCRIPTS_INDEX.hydra_account_balance.mint,
            address,
            blueprint.cbor,
            blueprint.hash,
        )
    }

    /// Hydra account balance spending script
    pub fn hydra_account_balance_spend(tx_hash: String, address: String) -> UTxO {
        let blueprint = hydra_account_balance_spending_blueprint();
        create_ref_script_utxo(
            tx_hash,
            L2_REF_SCRIPTS_INDEX.hydra_account_balance.spend,
            address,
            blueprint.cbor,
            blueprint.hash,
        )
    }
}
