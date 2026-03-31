use std::collections::HashMap;

use super::add_hyphens_to_map_keys;

pub struct IdTxIndexMap {
    pub map: HashMap<String, u32>,
    pub current_index: u32,
}

impl IdTxIndexMap {
    pub fn new(length: usize) -> Self {
        IdTxIndexMap {
            map: HashMap::with_capacity(length),
            current_index: 0,
        }
    }

    pub fn add(&mut self, id: &str) {
        self.map.insert(id.to_string(), self.current_index);
        self.current_index += 1;
    }

    pub fn set_index(&mut self, index: u32) {
        self.current_index = index;
    }

    pub fn to_proto(self) -> HashMap<String, u32> {
        add_hyphens_to_map_keys(self.map)
    }
}
