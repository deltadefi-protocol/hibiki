use crate::{scripts::MValue, services};
use std::collections::BTreeMap;
use whisky::data::{ByteString, Int, Map};
use whisky::*;

pub fn to_proto_amount(assets: &[Asset]) -> Vec<services::Asset> {
    let mut converted_assets = vec![];
    for asset in assets {
        converted_assets.push(services::Asset {
            unit: asset.unit(),
            quantity: asset.quantity(),
        });
    }
    converted_assets
}

pub fn from_proto_amount(assets: &[services::Asset]) -> Vec<Asset> {
    let mut converted_assets = vec![];
    for asset in assets {
        converted_assets.push(Asset::new(asset.unit.clone(), asset.quantity.clone()));
    }
    converted_assets
}

pub fn assets_to_mvalue(assets: &[Asset]) -> MValue {
    let mut outer_map: BTreeMap<String, BTreeMap<String, Int>> = BTreeMap::new();

    for asset in assets {
        let unit = asset.unit();
        let quantity = asset.quantity().parse::<i128>().unwrap_or(0);

        let (policy_id, asset_name) = if unit == "lovelace" || unit == "" {
            ("".to_string(), "".to_string())
        } else if unit.len() >= 56 {
            let policy_hex = unit[0..56].to_string();
            let asset_hex = if unit.len() > 56 {
                unit[56..].to_string()
            } else {
                "".to_string()
            };
            (policy_hex, asset_hex)
        } else {
            // Invalid format, skip
            continue;
        };

        outer_map
            .entry(policy_id)
            .or_insert_with(BTreeMap::new)
            .insert(asset_name, Int::new(quantity));
    }

    // Convert BTreeMap to Vec of tuples with ByteString keys
    let outer_vec: Vec<(ByteString, Map<ByteString, Int>)> = outer_map
        .into_iter()
        .map(|(policy_id, inner_map)| {
            let inner_vec: Vec<(ByteString, Int)> = inner_map
                .into_iter()
                .map(|(asset_name, quantity)| (ByteString::new(&asset_name), quantity))
                .collect();
            (ByteString::new(&policy_id), Map::new(&inner_vec))
        })
        .collect();

    Map::new(&outer_vec)
}
