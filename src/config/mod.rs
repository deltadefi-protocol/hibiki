use std::env::var;
use crate::utils::gcp_secret_manager;
pub mod hydra;

pub struct AppConfig {
    pub network_id: String,
    pub app_owner_mnemonic: String,
    pub app_owner_vkey: String,
    pub fee_collector_mnemonic: String,
}

impl AppConfig {
    pub fn new() -> AppConfig {
        // Get network ID from environment variable
        let network_id = var("NETWORK_ID").unwrap_or("0".to_string());
        
        // Get owner verification key from environment variable
        let app_owner_vkey = var("OWNER_VKEY").unwrap_or("".to_string());
        
        // Try to get APP_OWNER_SEED_PHRASE from environment first, then fall back to GCP Secret Manager
        let app_owner_mnemonic = match var("APP_OWNER_SEED_PHRASE") {
            Ok(phrase) => convert_mnemonic_comma_to_space(&phrase),
            Err(_) => {
                // If not found in environment, try to get from GCP Secret Manager
                match gcp_secret_manager::get_app_owner_seed_phrase() {
                    Ok(phrase) => convert_mnemonic_comma_to_space(&phrase),
                    Err(e) => {
                        eprintln!("Failed to get APP_OWNER_SEED_PHRASE: {}", e);
                        panic!("APP_OWNER_SEED_PHRASE not found in environment or Secret Manager");
                    }
                }
            }
        };
        
        // Try to get FEE_COLLECTOR_SEED_PHRASE from environment first, then fall back to GCP Secret Manager
        let fee_collector_mnemonic = match var("FEE_COLLECTOR_SEED_PHRASE") {
            Ok(phrase) => convert_mnemonic_comma_to_space(&phrase),
            Err(_) => {
                // If not found in environment, try to get from GCP Secret Manager
                match gcp_secret_manager::get_fee_collector_seed_phrase() {
                    Ok(phrase) => convert_mnemonic_comma_to_space(&phrase),
                    Err(e) => {
                        eprintln!("Failed to get FEE_COLLECTOR_SEED_PHRASE: {}", e);
                        panic!("FEE_COLLECTOR_SEED_PHRASE not found in environment or Secret Manager");
                    }
                }
            }
        };
        
        AppConfig {
            network_id,
            app_owner_mnemonic,
            app_owner_vkey,
            fee_collector_mnemonic,
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