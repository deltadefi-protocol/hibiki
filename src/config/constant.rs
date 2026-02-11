use crate::scripts::bar::*;
use crate::utils::token::hydra_to_l1_token_map;
use std::collections::HashMap;
use std::sync::OnceLock;

static USDM_UNIT: OnceLock<String> = OnceLock::new();
pub fn usdm_unit() -> &'static str {
    USDM_UNIT
        .get_or_init(|| std::env::var("USDM_UNIT").expect("USDM_UNIT must be set in environment"))
}

static NIGHT_UNIT: OnceLock<String> = OnceLock::new();
pub fn night_unit() -> &'static str {
    NIGHT_UNIT
        .get_or_init(|| std::env::var("NIGHT_UNIT").expect("NIGHT_UNIT must be set in environment"))
}

static IAG_UNIT: OnceLock<String> = OnceLock::new();
pub fn iag_unit() -> &'static str {
    IAG_UNIT.get_or_init(|| std::env::var("IAG_UNIT").expect("IAG_UNIT must be set in environment"))
}

static SNEK_UNIT: OnceLock<String> = OnceLock::new();
pub fn snek_unit() -> &'static str {
    SNEK_UNIT
        .get_or_init(|| std::env::var("SNEK_UNIT").expect("SNEK_UNIT must be set in environment"))
}

static HOSKY_UNIT: OnceLock<String> = OnceLock::new();
pub fn hosky_unit() -> &'static str {
    HOSKY_UNIT
        .get_or_init(|| std::env::var("HOSKY_UNIT").expect("HOSKY_UNIT must be set in environment"))
}

static DEX_ORACLE_NFT: OnceLock<String> = OnceLock::new();
pub fn dex_oracle_nft() -> &'static str {
    DEX_ORACLE_NFT.get_or_init(|| {
        std::env::var("DEX_ORACLE_NFT").expect("DEX_ORACLE_NFT must be set in environment")
    })
}

static HYDRA_TOKEN_HASH: OnceLock<String> = OnceLock::new();
pub fn hydra_token_hash() -> &'static str {
    HYDRA_TOKEN_HASH.get_or_init(|| {
        let policy_id = whisky::data::PolicyId::new(dex_oracle_nft());
        hydra_tokens_mint_minting_blueprint(&policy_id).hash
    })
}

static ALL_HYDRA_TO_L1_TOKEN_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
pub fn all_hydra_to_l1_token_map() -> &'static HashMap<String, String> {
    ALL_HYDRA_TO_L1_TOKEN_MAP.get_or_init(|| {
        hydra_to_l1_token_map(&[
            "",
            "lovelace",
            usdm_unit(),
            night_unit(),
            iag_unit(),
            snek_unit(),
            hosky_unit(),
        ])
    })
}
