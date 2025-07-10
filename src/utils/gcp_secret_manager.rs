use gouth::{Builder};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use std::env::var;
use std::error::Error;
use base64::Engine;

// Define structures to match the Google Cloud Secret Manager API response
#[derive(Debug, Deserialize)]
struct SecretPayload {
    data: String,
}

#[derive(Debug, Deserialize)]
struct AccessSecretVersionResponse {
    _name: String,
    payload: SecretPayload,
}

/// Access a secret version from Google Cloud Secret Manager using GKE Workload Identity and Gouth
/// 
/// # Arguments
/// * `project_id` - Google Cloud project ID
/// * `secret_id` - Name of the secret
/// * `version_id` - Version of the secret (numeric or "latest")
/// 
/// # Returns
/// The secret value as a String if successful
pub fn access_secret_version(
    project_id: &str, 
    secret_id: &str, 
    version_id: &str
) -> Result<String, Box<dyn Error>> {
    // Get credentials and token using gouth, which supports Application Default Credentials
    let token = Builder::new()
        .scopes(&["https://www.googleapis.com/auth/cloud-platform"])
        .build()?;
    
    // Build URL for the secret version access API
    let url = format!(
        "https://secretmanager.googleapis.com/v1/projects/{}/secrets/{}/versions/{}:access",
        project_id, secret_id, version_id
    );
    
    // Create HTTP client
    let client = Client::new();
    
    // Set up headers with authentication token
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&token.header_value()?)?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    
    // Send request to Google Cloud Secret Manager
    let response = client
        .get(&url)
        .headers(headers)
        .send()?;
    
    if !response.status().is_success() {
        return Err(format!("Failed to access secret version: Status: {}, Body: {}", 
            response.status(), response.text()?).into());
    }
    
    // Parse the response according to the API documentation
    let secret_response: AccessSecretVersionResponse = response.json()?;
    
    // Decode base64-encoded secret value
    let decoded = base64::engine::general_purpose::STANDARD.decode(&secret_response.payload.data)?;
    let secret_value = String::from_utf8(decoded)?;
    
    Ok(secret_value)
}

/// Get APP_OWNER_SEED_PHRASE from Google Cloud Secret Manager using CircleCI environment variables
/// 
/// # Returns
/// The secret value as a String if successful
pub fn get_app_owner_seed_phrase() -> Result<String, Box<dyn Error>> {
    // Get project ID from CircleCI env var
    let project_id = var("APP_OWNER_SEED_PHRASE_SECRET_MANAGER_PROJECT_ID").map_err(|_| {
        "APP_OWNER_SEED_PHRASE_SECRET_MANAGER_PROJECT_ID not set in CircleCI environment".to_string()
    })?;
    
    // Get secret ID from CircleCI env var
    let secret_id = var("APP_OWNER_SEED_PHRASE_SECRET_MANAGER_SECRET_ID").map_err(|_| {
        "APP_OWNER_SEED_PHRASE_SECRET_MANAGER_SECRET_ID not set in CircleCI environment".to_string()
    })?;
    
    // Get version ID from CircleCI env var (defaults to "latest")
    let version_id = var("APP_OWNER_SEED_PHRASE_SECRET_MANAGER_VERSION_ID")
        .unwrap_or_else(|_| "latest".to_string());
    
    // Access the secret version
    access_secret_version(&project_id, &secret_id, &version_id)
}

/// Get FEE_COLLECTOR_SEED_PHRASE from Google Cloud Secret Manager using CircleCI environment variables
/// 
/// # Returns
/// The secret value as a String if successful
pub fn get_fee_collector_seed_phrase() -> Result<String, Box<dyn Error>> {
    // Get project ID from CircleCI env var
    let project_id = var("FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_PROJECT_ID").map_err(|_| {
        "FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_PROJECT_ID not set in CircleCI environment".to_string()
    })?;
    
    // Get secret ID from CircleCI env var
    let secret_id = var("FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_SECRET_ID").map_err(|_| {
        "FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_SECRET_ID not set in CircleCI environment".to_string()
    })?;
    
    // Get version ID from CircleCI env var (defaults to "latest")
    let version_id = var("FEE_COLLECTOR_SEED_PHRASE_SECRET_MANAGER_VERSION_ID")
        .unwrap_or_else(|_| "latest".to_string());
    
    // Access the secret version
    access_secret_version(&project_id, &secret_id, &version_id)
}