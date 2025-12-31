use crate::scripts::bar::*;
use crate::utils::token::hydra_to_l1_token_map;
use std::collections::HashMap;
use std::sync::OnceLock;
use whisky::data::{OutputReference, PolicyId};

static USDM_UNIT: OnceLock<String> = OnceLock::new();
pub fn usdm_unit() -> &'static str {
    USDM_UNIT
        .get_or_init(|| std::env::var("USDM_UNIT").expect("USDM_UNIT must be set in environment"))
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
    ALL_HYDRA_TO_L1_TOKEN_MAP.get_or_init(|| hydra_to_l1_token_map(&["", usdm_unit()]))
}

/// Script blueprints organized by function
pub struct Scripts {
    pub dex_order_book: DexOrderBookScripts,
    pub hydra_user_intent: HydraUserIntentScripts,
    pub hydra_account_balance: HydraAccountBalanceScripts,
    pub hydra_order_book: HydraOrderBookScripts,
    pub hydra_token: HydraTokenScripts,
}

pub struct DexOrderBookScripts {
    pub mint: fn(OutputReference) -> whisky::MintingBlueprint<OutputReference, MintPolarity>,
    pub spend: fn(
        (PolicyId, PolicyId),
    ) -> whisky::SpendingBlueprint<
        (PolicyId, PolicyId),
        DexOrderBookRedeemer,
        DexOrderBookDatum,
    >,
}

pub struct HydraUserIntentScripts {
    pub mint: fn(&PolicyId) -> whisky::MintingBlueprint<PolicyId, HydraUserIntentRedeemer>,
    pub spend:
        fn(
            &PolicyId,
        )
            -> whisky::SpendingBlueprint<PolicyId, whisky::data::PlutusData, HydraUserIntentDatum>,
}

pub struct HydraAccountBalanceScripts {
    pub spend:
        fn(&PolicyId) -> whisky::SpendingBlueprint<PolicyId, HydraAccountRedeemer, UserAccount>,
    pub withdrawal: fn(&PolicyId) -> whisky::WithdrawalBlueprint<PolicyId, HydraAccountOperation>,
}

pub struct HydraOrderBookScripts {
    pub spend:
        fn(&PolicyId) -> whisky::SpendingBlueprint<PolicyId, whisky::data::PlutusData, Order>,
    pub withdrawal: fn(&PolicyId) -> whisky::WithdrawalBlueprint<PolicyId, HydraOrderBookRedeemer>,
}

pub struct HydraTokenScripts {
    pub mint: fn(&PolicyId) -> whisky::MintingBlueprint<PolicyId, HydraTokensRedeemer>,
}

pub const SCRIPTS: Scripts = Scripts {
    dex_order_book: DexOrderBookScripts {
        mint: oracle_nft_mint_minting_blueprint,
        spend: dex_order_book_spend_spending_blueprint,
    },
    hydra_user_intent: HydraUserIntentScripts {
        mint: hydra_user_intent_mint_minting_blueprint,
        spend: hydra_user_intent_spend_spending_blueprint,
    },
    hydra_account_balance: HydraAccountBalanceScripts {
        spend: hydra_account_spend_spending_blueprint,
        withdrawal: hydra_account_withdraw_withdrawal_blueprint,
    },
    hydra_order_book: HydraOrderBookScripts {
        spend: hydra_order_book_spend_spending_blueprint,
        withdrawal: hydra_order_book_withdraw_withdrawal_blueprint,
    },
    hydra_token: HydraTokenScripts {
        mint: hydra_tokens_mint_minting_blueprint,
    },
};

/// L2 (Hydra) reference script indices
/// These correspond to the output indices of reference scripts published on Hydra
pub mod l2_ref_scripts_index {
    pub mod dex_order_book {
        pub const SPEND: u32 = 2;
    }

    pub mod hydra_user_intent {
        pub const MINT: u32 = 3;
        pub const SPEND: u32 = 4;
    }

    pub mod hydra_account_balance {
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
