use blake2::{
    digest::{Update, VariableOutput},
    Blake2bVar,
};
use std::sync::OnceLock;
use whisky::Asset;

static HYDRA_TOKEN_HASH: OnceLock<String> = OnceLock::new();

pub fn hydra_token_hash() -> &'static str {
    HYDRA_TOKEN_HASH.get_or_init(|| {
        std::env::var("HYDRA_TOKEN_HASH").unwrap_or_else(|_| {
            "c828db378a1b202822e9de2a6d461af04b016768bce986176af87ba5".to_string()
        })
    })
}

pub fn to_hydra_token(assets: &[Asset]) -> Vec<Asset> {
    let hydra_token_hash = hydra_token_hash();

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

    #[test]
    fn test_to_hydra_token_lovelace() {
        let assets = vec![Asset::new_from_str("lovelace", "1000000")];
        let hydra_assets = to_hydra_token(&assets);

        assert_eq!(hydra_assets.len(), 1);
        assert!(hydra_assets[0].unit().len() == 56);
        assert_eq!(hydra_assets[0].quantity(), "1000000");
    }

    #[test]
    fn test_to_hydra_token_custom_asset() {
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
}
