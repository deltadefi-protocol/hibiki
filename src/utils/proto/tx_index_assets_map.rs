use std::collections::HashMap;

use hibiki_proto::services::{AssetList, UnitTxIndexMap};
use whisky::Asset;

use super::{add_hyphens_to_map_keys, to_proto_amount};

pub struct TxIndexAssetsMap {
    pub map: HashMap<String, AssetList>,
    pub current_index: u32,
}

impl TxIndexAssetsMap {
    pub fn new(length: usize) -> Self {
        TxIndexAssetsMap {
            map: HashMap::with_capacity(length),
            current_index: 0,
        }
    }

    pub fn insert(&mut self, assets: &[Asset]) {
        self.map.insert(
            self.current_index.to_string(),
            AssetList {
                assets: to_proto_amount(assets),
            },
        );
        self.current_index += 1;
    }

    pub fn set_index(&mut self, index: u32) {
        self.current_index = index;
    }

    pub fn to_proto(self) -> Option<UnitTxIndexMap> {
        if self.map.is_empty() {
            None
        } else {
            Some(UnitTxIndexMap {
                unit_tx_index_map: self.map,
            })
        }
    }
}

impl Default for TxIndexAssetsMap {
    fn default() -> Self {
        TxIndexAssetsMap {
            map: HashMap::new(),
            current_index: 0,
        }
    }
}

pub struct AccountTxIndexAssetsMap {
    accounts: HashMap<String, HashMap<String, AssetList>>,
    pub current_index: u32,
}

impl AccountTxIndexAssetsMap {
    pub fn new() -> Self {
        AccountTxIndexAssetsMap {
            accounts: HashMap::new(),
            current_index: 0,
        }
    }

    pub fn insert(&mut self, account_id: &str, assets: &[Asset]) {
        let entry = self
            .accounts
            .entry(account_id.to_string())
            .or_insert_with(HashMap::new);

        entry.insert(
            self.current_index.to_string(),
            AssetList {
                assets: to_proto_amount(assets),
            },
        );
        self.current_index += 1;
    }

    pub fn to_proto(self) -> HashMap<String, UnitTxIndexMap> {
        let map: HashMap<String, UnitTxIndexMap> = self
            .accounts
            .into_iter()
            .map(|(account_id, tx_index_map)| {
                (
                    account_id,
                    UnitTxIndexMap {
                        unit_tx_index_map: tx_index_map,
                    },
                )
            })
            .collect();
        add_hyphens_to_map_keys(map)
    }
}
