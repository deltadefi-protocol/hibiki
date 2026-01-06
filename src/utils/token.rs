use blake2::{
    digest::{Update, VariableOutput},
    Blake2bVar,
};
use std::collections::HashMap;
use whisky::Asset;

pub fn hydra_to_l1_token_map(units: &[&str]) -> HashMap<String, String> {
    let hydra_token_hash = crate::constant::hydra_token_hash();
    let mut map = HashMap::with_capacity(units.len());

    for unit in units {
        if *unit == "lovelace" || unit.is_empty() {
            map.insert(hydra_token_hash.to_string(), "lovelace".to_string());
        } else {
            let hashed_unit = blake2b_256_hex(unit);
            let hydra_unit = format!("{}{}", hydra_token_hash, hashed_unit);
            map.insert(hydra_unit, unit.to_string());
        }
    }

    map
}

pub fn to_l1_assets(
    assets: &[Asset],
    hydra_to_l1_map: &HashMap<String, String>,
) -> Result<Vec<Asset>, String> {
    assets
        .iter()
        // Filter out lovelace (hydra token hash) and empty units before conversion
        .filter(|asset| {
            let unit = asset.unit();
            !unit.is_empty() && unit != "lovelace"
        })
        .map(|asset| {
            match hydra_to_l1_map.get(&asset.unit()) {
                Some(l1_unit) => Ok(Asset::new_from_str(l1_unit, &asset.quantity())),
                None => Err(format!(
                    "Unknown Hydra token unit: {}. Please ensure it's registered in hydra_to_l1_token_map.",
                    asset.unit()
                )),
            }
        })
        .collect()
}

pub fn to_hydra_token(assets: &[Asset]) -> Vec<Asset> {
    let hydra_token_hash = crate::constant::hydra_token_hash();

    assets
        .iter()
        .map(|asset| {
            let new_unit = if asset.unit() == "lovelace" || asset.unit().is_empty() {
                hydra_token_hash.to_string()
            } else {
                let hashed_unit = blake2b_256_hex(&asset.unit());
                let mut result = String::with_capacity(hydra_token_hash.len() + hashed_unit.len());
                result.push_str(hydra_token_hash);
                result.push_str(&hashed_unit);
                result
            };

            Asset::new_from_str(&new_unit, &asset.quantity())
        })
        .collect()
}

/// Hash a hex string using Blake2b-256 and return the hex result
pub fn blake2b_256_hex(hex_input: &str) -> String {
    let input_bytes = hex::decode(hex_input).unwrap_or_else(|_| hex_input.as_bytes().to_vec());

    let mut hasher = Blake2bVar::new(32).expect("Invalid Blake2b output size");
    hasher.update(&input_bytes);

    let mut result = vec![0u8; 32];
    hasher
        .finalize_variable(&mut result)
        .expect("Failed to finalize hash");
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_test_env;

    #[test]
    fn test_to_hydra_token_lovelace() {
        init_test_env();
        let assets = vec![Asset::new_from_str("lovelace", "1000000")];
        let hydra_assets = to_hydra_token(&assets);

        assert_eq!(hydra_assets.len(), 1);
        assert!(hydra_assets[0].unit().len() == 56);
        assert_eq!(hydra_assets[0].quantity(), "1000000");
    }

    #[test]
    fn test_to_hydra_token_custom_asset() {
        init_test_env();
        let assets = vec![Asset::new_from_str(
            "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
            "100",
        )];
        let hydra_assets = to_hydra_token(&assets);

        assert_eq!(hydra_assets.len(), 1);
        assert!(hydra_assets[0].unit().len() > 56);
        assert_eq!(hydra_assets[0].quantity(), "100");
    }

    #[test]
    fn test_blake2b_256_hex() {
        let input = "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d";
        let hash = blake2b_256_hex(input);

        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "ae67ab5990f1d43f7f7ed7916888deeef55b8b27d4d155a2c6192601f1566f4e"
        );
    }

    #[test]
    fn test_to_hydra_token_multiple_assets() {
        init_test_env();
        let assets = vec![
            Asset::new_from_str("lovelace", "1000000"),
            Asset::new_from_str(
                "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
                "100",
            ),
        ];
        let hydra_assets = to_hydra_token(&assets);

        assert_eq!(hydra_assets.len(), 2);
        assert_eq!(hydra_assets[0].unit().len(), 56);
        assert!(hydra_assets[1].unit().len() > 56);
    }

    #[test]
    fn test_hydra_to_l1_token_map() {
        init_test_env();
        let units = vec![
            "",
            "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d",
        ];
        let map = hydra_to_l1_token_map(&units);

        assert_eq!(map.len(), 2);
        // Check that lovelace mapping exists
        let hydra_token_hash = crate::constant::hydra_token_hash();
        assert_eq!(map.get(hydra_token_hash), Some(&"lovelace".to_string()));
    }

    #[test]
    fn test_to_l1_assets() {
        init_test_env();
        let usdm_unit = "c69b981db7a65e339a6d783755f85a2e03afa1cece9714c55fe4c9135553444d";
        let units = vec!["", usdm_unit];
        let map = hydra_to_l1_token_map(&units);

        // Convert L1 assets to hydra assets
        let l1_assets = vec![
            Asset::new_from_str("lovelace", "1000000"),
            Asset::new_from_str(usdm_unit, "500"),
        ];
        let hydra_assets = to_hydra_token(&l1_assets);

        // Convert back to L1 assets (lovelace is now filtered out)
        let result = to_l1_assets(&hydra_assets, &map);
        assert!(result.is_ok());

        let converted = result.unwrap();
        println!("Converted assets: {:?}", converted);
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].unit(), "lovelace");
        assert_eq!(converted[0].quantity(), "1000000");
    }

    #[test]
    fn test_to_l1_assets_unknown_unit() {
        init_test_env();

        let map = hydra_to_l1_token_map(&[""]); // Only lovelace registered
        let unknown_hydra_assets = vec![Asset::new_from_str("unknown_hash_12345", "100")];

        let result = to_l1_assets(&unknown_hydra_assets, &map);
        assert!(result.is_err());
    }
}
