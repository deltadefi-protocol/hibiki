use std::env::var;

pub struct AppConfig {
    pub network_id: String,
    pub app_owner_mnemonic: String,
}

impl AppConfig {
    pub fn new() -> AppConfig {
        AppConfig {
            network_id: var("NETWORK_ID").unwrap_or("0".to_string()),
            app_owner_mnemonic: var("APP_OWNER_SEED_PHRASE").unwrap(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new()
    }
}
