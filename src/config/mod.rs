use std::env::var;
pub mod hydra;

pub struct AppConfig {
    pub network_id: String,
    pub app_owner_mnemonic: String,
    pub app_owner_vkey: String,
}

impl AppConfig {
    pub fn new() -> AppConfig {
        AppConfig {
            network_id: var("NETWORK_ID").unwrap_or("0".to_string()),
            app_owner_mnemonic: convert_mnemonic_comma_to_space(
                &var("APP_OWNER_SEED_PHRASE").unwrap(),
            ),
            app_owner_vkey: var("OWNER_VKEY").unwrap_or("".to_string()),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new()
    }
}

fn convert_mnemonic_comma_to_space(mnemonic: &str) -> String {
    mnemonic.replace(',', " ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_mnemonic_comma_conversion() {
        let mnemonic = "solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution,solution";
        let expected_mnemonic = "solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution solution";
        let converted = super::convert_mnemonic_comma_to_space(mnemonic);
        assert_eq!(converted, expected_mnemonic);
    }
}
