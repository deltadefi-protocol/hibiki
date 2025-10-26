pub struct AccountOperationScripts {
    pub hydra_head_open: u32,
    pub hydra_head_close: u32,
    pub hydra_internal_transfer: u32,
    pub hydra_withdrawal: u32,
    pub hydra_combine_balance: u32,
}

pub struct DexAccountBalanceScripts {
    pub spend: u32,
}

pub struct DexNetDepositScripts {
    pub spend: u32,
}

pub struct DexOrderBookScripts {
    pub spend: u32,
    pub combine_withdraw: u32,
    pub split_withdraw: u32,
}

pub struct HydraUserIntentScripts {
    pub mint: u32,
    pub spend: u32,
}

pub struct HydraAccountBalanceScripts {
    pub mint: u32,
    pub spend: u32,
}

pub struct HydraOrderBookScripts {
    pub mint: u32,
    pub spend: u32,
    pub cancel_withdraw: u32,
    pub fill_withdraw: u32,
    pub place_withdraw: u32,
}

pub struct L2RefScriptsIndex {
    pub account_operation: AccountOperationScripts,
    pub dex_account_balance: DexAccountBalanceScripts,
    pub dex_net_deposit: DexNetDepositScripts,
    pub dex_order_book: DexOrderBookScripts,
    pub hydra_user_intent: HydraUserIntentScripts,
    pub hydra_account_balance: HydraAccountBalanceScripts,
    pub hydra_order_book: HydraOrderBookScripts,
}

pub const L2_REF_SCRIPTS_INDEX: L2RefScriptsIndex = L2RefScriptsIndex {
    account_operation: AccountOperationScripts {
        hydra_head_open: 1001,
        hydra_head_close: 1002,
        hydra_internal_transfer: 1003,
        hydra_withdrawal: 1004,
        hydra_combine_balance: 1005,
    },
    dex_account_balance: DexAccountBalanceScripts { spend: 1006 },
    dex_net_deposit: DexNetDepositScripts { spend: 1007 },
    dex_order_book: DexOrderBookScripts {
        spend: 1008,
        combine_withdraw: 1009,
        split_withdraw: 1010,
    },
    hydra_user_intent: HydraUserIntentScripts {
        mint: 1011,
        spend: 1012,
    },
    hydra_account_balance: HydraAccountBalanceScripts {
        mint: 1013,
        spend: 1014,
    },
    hydra_order_book: HydraOrderBookScripts {
        mint: 1015,
        spend: 1016,
        cancel_withdraw: 1017,
        fill_withdraw: 1018,
        place_withdraw: 1019,
    },
};
