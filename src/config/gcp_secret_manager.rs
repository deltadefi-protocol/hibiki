use std::env::var;
use std::error::Error;
use crate::utils::gcp_secret_manager::access_secret_version;

/// Get APP_OWNER_SEED_PHRASE from Google Cloud Secret Manager using environment variables
/// 
/// # Returns
/// The secret value as a String if successful
pub fn get_app_owner_seed_phrase() -> Result<String, Box<dyn Error>> {
    let project_id = var("APP_OWNER_SEED_PHRASE_SECRET_MANAGER_PROJECT_ID").map_err(|_| {
        "APP_OWNER_SEED_PHRASE_SECRET_MANAGER_PROJECT_ID not set in environment".to_string()
    })?;
    
    let secret_id = var("APP_OWNER_SEED_PHRASE_SECRET_MANAGER_SECRET_ID").map_err(|_| {
        "APP_OWNER_SEED_PHRASE_SECRET_MANAGER_SECRET_ID not set in environment".to_string()
    })?;
    
    let version_id = var("APP_OWNER_SEED_PHRASE_SECRET_MANAGER_VERSION_ID")
        .unwrap_or_else(|_| "latest".to_string());
    
    access_secret_version(&project_id, &secret_id, &version_id)
}

/// Get FEE_COLLECTOR_SEED_PHRASE from Google Cloud Secret Manager using environment variables
/// 
/// # Returns
/// The secret value as a String if successful
pub fn get_fee_collector_seed_phrase() -> Result<String, Box<dyn Error>> {
    let project_id = var("FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_PROJECT_ID").map_err(|_| {
        "FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_PROJECT_ID not set in environment".to_string()
    })?;
    
    let secret_id = var("FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_SECRET_ID").map_err(|_| {
        "FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_SECRET_ID not set in environment".to_string()
    })?;
    
    let version_id = var("FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_VERSION_ID")
        .unwrap_or_else(|_| "latest".to_string());
    
    access_secret_version(&project_id, &secret_id, &version_id)
}