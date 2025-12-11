use crate::scripts::bar::*;
use std::sync::OnceLock;
use whisky::data::{OutputReference, PolicyId};

/// DEX Oracle NFT policy ID (loaded once from environment)
static DEX_ORACLE_NFT: OnceLock<String> = OnceLock::new();

/// Get the DEX Oracle NFT policy ID from environment (cached after first call)
pub fn dex_oracle_nft() -> &'static str {
    DEX_ORACLE_NFT.get_or_init(|| {
        std::env::var("DEX_ORACLE_NFT").expect("DEX_ORACLE_NFT must be set in environment")
    })
}

/// Script blueprints organized by function
pub struct Scripts {
    pub app_oracle: AppOracleScripts,
    pub app_vault: AppVaultScripts,
    pub app_deposit_request: AppDepositRequestScripts,
    pub emergency_request: EmergencyRequestScripts,
    pub dex_account_balance: DexAccountBalanceScripts,
    pub dex_order_book: DexOrderBookScripts,
    pub hydra_user_intent: HydraUserIntentScripts,
    pub hydra_account_balance: HydraAccountBalanceScripts,
    pub hydra_order_book: HydraOrderBookScripts,
    pub hydra_token: HydraTokenScripts,
}

pub struct AppOracleScripts {
    pub mint: fn(OutputReference) -> whisky::MintingBlueprint<OutputReference, MintPolarity>,
    pub spend: fn() -> whisky::SpendingBlueprint<(), AppOracleRedeemer, AppOracleDatum>,
}

pub struct AppVaultScripts {
    pub spend: fn(
        &PolicyId,
    ) -> whisky::SpendingBlueprint<
        PolicyId,
        whisky::data::PlutusData,
        whisky::data::PlutusData,
    >,
    pub withdrawal:
        fn(&PolicyId) -> whisky::WithdrawalBlueprint<PolicyId, ProcessAppWithdrawalRedeemer>,
}

pub struct AppDepositRequestScripts {
    pub mint: fn(&PolicyId) -> whisky::MintingBlueprint<PolicyId, MintPolarity>,
    pub spend: fn(
        &PolicyId,
    ) -> whisky::SpendingBlueprint<
        PolicyId,
        AppDepositRequestRedeemer,
        AppDepositRequestDatum,
    >,
    pub withdrawal: fn(&PolicyId) -> whisky::WithdrawalBlueprint<PolicyId, ProcessAppDeposit>,
}

pub struct EmergencyRequestScripts {
    pub cancel_order_mint: fn(&PolicyId) -> whisky::MintingBlueprint<PolicyId, MintPolarity>,
    pub cancel_order_spend: fn(
        &PolicyId,
    ) -> whisky::SpendingBlueprint<
        PolicyId,
        EmergencyCancelRequestRedeemer,
        EmergencyCancelRequestDatum,
    >,
    pub withdrawal_mint: fn(&PolicyId) -> whisky::MintingBlueprint<PolicyId, MintPolarity>,
    pub withdrawal_spend: fn(
        &PolicyId,
    ) -> whisky::SpendingBlueprint<
        PolicyId,
        EmergencyWithdrawalRequestRedeemer,
        EmergencyWithdrawalRequestDatum,
    >,
}

pub struct DexAccountBalanceScripts {
    pub mint: fn(&PolicyId) -> whisky::MintingBlueprint<PolicyId, MintPolarity>,
    pub spend: fn(
        (PolicyId, PolicyId),
    ) -> whisky::SpendingBlueprint<
        (PolicyId, PolicyId),
        DexAccountBalanceRedeemer,
        DexAccountBalanceDatum,
    >,
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
    pub emergency_cancel:
        fn(&PolicyId) -> whisky::WithdrawalBlueprint<PolicyId, EmergencyCancelRedeemer>,
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
    app_oracle: AppOracleScripts {
        mint: oracle_nft_mint_minting_blueprint,
        spend: app_oracle_spend_spending_blueprint,
    },
    app_vault: AppVaultScripts {
        spend: app_vault_spend_spending_blueprint,
        withdrawal: app_withdrawal_withdraw_withdrawal_blueprint,
    },
    app_deposit_request: AppDepositRequestScripts {
        mint: app_deposit_request_mint_minting_blueprint,
        spend: app_deposit_request_spend_spending_blueprint,
        withdrawal: app_deposit_withdraw_withdrawal_blueprint,
    },
    emergency_request: EmergencyRequestScripts {
        cancel_order_mint: emergency_cancel_order_request_mint_minting_blueprint,
        cancel_order_spend: emergency_cancel_order_request_spend_spending_blueprint,
        withdrawal_mint: emergency_withdrawal_request_mint_minting_blueprint,
        withdrawal_spend: emergency_withdrawal_request_spend_spending_blueprint,
    },
    dex_account_balance: DexAccountBalanceScripts {
        mint: dex_account_balance_mint_minting_blueprint,
        spend: dex_account_balance_spend_spending_blueprint,
    },
    dex_order_book: DexOrderBookScripts {
        mint: oracle_nft_mint_minting_blueprint,
        spend: dex_order_book_spend_spending_blueprint,
        emergency_cancel: emergency_order_cancel_withdraw_withdrawal_blueprint,
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
    pub mod dex_account_balance {
        pub const SPEND: u32 = 1;
    }

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
