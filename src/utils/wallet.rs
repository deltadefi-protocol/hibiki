use crate::config::AppConfig;
use whisky::Wallet;

pub fn get_app_owner_wallet() -> Wallet {
    let app_config = AppConfig::new();
    let owner_mnemonic = app_config.app_owner_mnemonic;
    Wallet::new_mnemonic(&owner_mnemonic).expect("Failed to create app owner wallet")
}

pub fn get_fee_collector_wallet() -> Wallet {
    let app_config = AppConfig::new();
    let owner_mnemonic = app_config.fee_collector_mnemonic;
    Wallet::new_mnemonic(&owner_mnemonic).expect("Failed to create fee collector wallet")
}
