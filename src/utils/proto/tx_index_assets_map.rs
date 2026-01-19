use hibiki_proto::services::UnitTxIndexMap;
use std::collections::HashMap;
use whisky::Asset;

use hibiki_proto::services::AssetList;

use crate::utils::proto::to_proto_amount;

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
