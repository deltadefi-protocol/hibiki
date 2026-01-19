use std::collections::HashMap;

/// Adds hyphens back to a UUID string.
///
/// # Arguments
/// * `uuid` - The UUID string without hyphens (32 chars)
///
/// # Returns
/// The UUID string with hyphens (36 chars): xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
pub fn add_hyphens(uuid: &str) -> String {
    if uuid.len() != 32 {
        return uuid.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &uuid[0..8],
        &uuid[8..12],
        &uuid[12..16],
        &uuid[16..20],
        &uuid[20..32]
    )
}

/// Transforms a HashMap by adding hyphens to all UUID keys.
pub fn add_hyphens_to_map_keys<V>(map: HashMap<String, V>) -> HashMap<String, V> {
    map.into_iter()
        .map(|(key, value)| (add_hyphens(&key), value))
        .collect()
}
