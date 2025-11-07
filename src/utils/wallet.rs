use crate::config::AppConfig;
use whisky::{NetworkId, Wallet};

pub fn get_app_owner_wallet() -> Wallet {
    let app_config = AppConfig::new();
    let owner_mnemonic = app_config.app_owner_mnemonic;
    let network_id = match app_config
        .network_id
        .parse::<u8>()
        .expect("Failed to parse network_id")
    {
        0 => NetworkId::Preprod,
        1 => NetworkId::Mainnet,
        _ => NetworkId::Preprod, // Default to Preprod
    };

    Wallet::new_mnemonic(&owner_mnemonic)
        .expect("Failed to create app owner wallet")
        .with_network_id(network_id)
}

pub fn get_fee_collector_wallet() -> Wallet {
    let app_config = AppConfig::new();
    let owner_mnemonic = app_config.fee_collector_mnemonic;
    let network_id = match app_config
        .network_id
        .parse::<u8>()
        .expect("Failed to parse network_id")
    {
        0 => NetworkId::Preprod,
        1 => NetworkId::Mainnet,
        _ => NetworkId::Preprod, // Default to Preprod
    };

    Wallet::new_mnemonic(&owner_mnemonic)
        .expect("Failed to create fee collector wallet")
        .with_network_id(network_id)
}
