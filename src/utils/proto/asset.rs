use crate::services;
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
