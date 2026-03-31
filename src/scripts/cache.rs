use whisky::data::{PlutusDataJson, PolicyId};
use whisky::{Budget, UTxO, UtxoInput, UtxoOutput, WData, WError, WRedeemer};

use super::bar::{
    hydra_account_spend_spending_blueprint, hydra_account_withdraw_withdrawal_blueprint,
    hydra_order_book_spend_spending_blueprint, hydra_order_book_withdraw_withdrawal_blueprint,
    hydra_tokens_mint_minting_blueprint, hydra_user_intent_mint_minting_blueprint,
    hydra_user_intent_spend_spending_blueprint,
};
use crate::config::constant::dex_oracle_nft;
use crate::utils::hydra::get_script_ref_hex;

fn build_script_ref_utxo(
    collateral: &UTxO,
    script_cbor: &str,
    script_hash: &str,
    output_index: u32,
) -> Result<UTxO, WError> {
    let script_ref_hex = Some(get_script_ref_hex(script_cbor)?);
    Ok(UTxO {
        input: UtxoInput {
            output_index,
            tx_hash: collateral.input.tx_hash.clone(),
        },
        output: UtxoOutput {
            address: collateral.output.address.clone(),
            amount: Vec::new(),
            data_hash: None,
            plutus_data: None,
            script_ref: script_ref_hex,
            script_hash: Some(script_hash.to_string()),
        },
    })
}

/// Cached script information for spending/withdrawal scripts
#[derive(Debug, Clone)]
pub struct CachedScript {
    pub hash: String,
    pub address: String,
    pub cbor: String,
    pub size: usize,
    pub ref_output_index: u32,
}

impl CachedScript {
    /// Encode a redeemer value to WData
    pub fn redeemer<R: PlutusDataJson>(&self, redeemer: R, budget: Option<Budget>) -> WRedeemer {
        WRedeemer {
            data: WData::JSON(redeemer.to_json_string()),
            ex_units: budget.unwrap_or_default(),
        }
    }

    ///  Get the reference UTxO for this script
    pub fn ref_utxo(&self, collateral: &UTxO) -> Result<UTxO, WError> {
        build_script_ref_utxo(collateral, &self.cbor, &self.hash, self.ref_output_index)
            .map_err(WError::from_err("CachedScript - ref_utxo"))
    }
}

/// Cached script information for minting scripts (no address)
#[derive(Debug, Clone)]
pub struct CachedMintScript {
    pub hash: String,
    pub cbor: String,
    pub size: usize,
    pub ref_output_index: u32,
}

impl CachedMintScript {
    /// Encode a redeemer value to WData
    pub fn redeemer<R: PlutusDataJson>(&self, redeemer: R, budget: Option<Budget>) -> WRedeemer {
        WRedeemer {
            data: WData::JSON(redeemer.to_json_string()),
            ex_units: budget.unwrap_or_default(),
        }
    }

    ///  Get the reference UTxO for this script
    pub fn ref_utxo(&self, collateral: &UTxO) -> Result<UTxO, WError> {
        build_script_ref_utxo(collateral, &self.cbor, &self.hash, self.ref_output_index)
            .map_err(WError::from_err("CachedScript - ref_utxo"))
    }
}

/// Pre-computed script cache initialized at startup
#[derive(Debug, Clone)]
pub struct ScriptCache {
    // Minting scripts
    pub user_intent_mint: CachedMintScript,
    pub hydra_token_mint: CachedMintScript,

    // Spending scripts
    pub user_intent_spend: CachedScript,
    pub hydra_account_spend: CachedScript,
    pub hydra_order_book_spend: CachedScript,

    // Withdrawal scripts
    pub hydra_account_withdrawal: CachedScript,
    pub hydra_order_book_withdrawal: CachedScript,
}

impl ScriptCache {
    /// Initialize the script cache with all pre-computed script info.
    /// This should be called once at startup.
    pub fn new() -> Self {
        let policy_id = PolicyId::new(dex_oracle_nft());

        // Minting scripts
        let user_intent_mint_bp = hydra_user_intent_mint_minting_blueprint(&policy_id);
        let hydra_token_mint_bp = hydra_tokens_mint_minting_blueprint(&policy_id);

        // Spending scripts
        let user_intent_spend_bp = hydra_user_intent_spend_spending_blueprint(&policy_id);
        let hydra_account_spend_bp = hydra_account_spend_spending_blueprint(&policy_id);
        let hydra_order_book_spend_bp = hydra_order_book_spend_spending_blueprint(&policy_id);

        // Withdrawal scripts
        let hydra_account_withdrawal_bp = hydra_account_withdraw_withdrawal_blueprint(&policy_id);
        let hydra_order_book_withdrawal_bp =
            hydra_order_book_withdraw_withdrawal_blueprint(&policy_id);

        Self {
            user_intent_mint: CachedMintScript {
                hash: user_intent_mint_bp.hash,
                size: user_intent_mint_bp.cbor.len() / 2,
                cbor: user_intent_mint_bp.cbor,
                ref_output_index: l2_ref_scripts_index::hydra_user_intent::MINT,
            },
            hydra_token_mint: CachedMintScript {
                hash: hydra_token_mint_bp.hash,
                size: hydra_token_mint_bp.cbor.len() / 2,
                cbor: hydra_token_mint_bp.cbor,
                ref_output_index: l2_ref_scripts_index::hydra_token::MINT,
            },
            user_intent_spend: CachedScript {
                hash: user_intent_spend_bp.hash,
                address: user_intent_spend_bp.address,
                size: user_intent_spend_bp.cbor.len() / 2,
                cbor: user_intent_spend_bp.cbor,
                ref_output_index: l2_ref_scripts_index::hydra_user_intent::SPEND,
            },
            hydra_account_spend: CachedScript {
                hash: hydra_account_spend_bp.hash,
                address: hydra_account_spend_bp.address,
                size: hydra_account_spend_bp.cbor.len() / 2,
                cbor: hydra_account_spend_bp.cbor,
                ref_output_index: l2_ref_scripts_index::hydra_account::SPEND,
            },
            hydra_order_book_spend: CachedScript {
                hash: hydra_order_book_spend_bp.hash,
                address: hydra_order_book_spend_bp.address,
                size: hydra_order_book_spend_bp.cbor.len() / 2,
                cbor: hydra_order_book_spend_bp.cbor,
                ref_output_index: l2_ref_scripts_index::hydra_order_book::SPEND,
            },
            hydra_account_withdrawal: CachedScript {
                hash: hydra_account_withdrawal_bp.hash,
                address: hydra_account_withdrawal_bp.address,
                size: hydra_account_withdrawal_bp.cbor.len() / 2,
                cbor: hydra_account_withdrawal_bp.cbor,
                ref_output_index: l2_ref_scripts_index::hydra_account::WITHDRAWAL,
            },
            hydra_order_book_withdrawal: CachedScript {
                hash: hydra_order_book_withdrawal_bp.hash,
                address: hydra_order_book_withdrawal_bp.address,
                size: hydra_order_book_withdrawal_bp.cbor.len() / 2,
                cbor: hydra_order_book_withdrawal_bp.cbor,
                ref_output_index: l2_ref_scripts_index::hydra_order_book::WITHDRAWAL,
            },
        }
    }
}

impl Default for ScriptCache {
    fn default() -> Self {
        Self::new()
    }
}

pub mod l2_ref_scripts_index {
    pub mod dex_order_book {
        pub const SPEND: u32 = 2;
    }

    pub mod hydra_user_intent {
        pub const MINT: u32 = 3;
        pub const SPEND: u32 = 4;
    }

    pub mod hydra_account {
        pub const SPEND: u32 = 5;
        pub const WITHDRAWAL: u32 = 6;
    }

    pub mod hydra_order_book {
        pub const SPEND: u32 = 7;
        pub const WITHDRAWAL: u32 = 8;
    }

    pub mod hydra_token {
        pub const MINT: u32 = 9;
    }
}
