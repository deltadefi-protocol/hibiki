use whisky::{get_mainnet_cost_models, Protocol};

pub fn get_hydra_pp() -> Protocol {
    Protocol {
        epoch: 0,
        min_fee_a: 0,
        min_fee_b: 0,
        max_block_size: 98_304_000,
        max_tx_size: 4_294_967_295,
        max_block_header_size: 1100,
        key_deposit: 0,
        pool_deposit: 0,
        min_pool_cost: "170000000".to_string(),
        price_mem: 0.0,
        price_step: 0.0,
        max_tx_ex_mem: "16000000000000000".to_string(),
        max_tx_ex_steps: "10000000000000000000".to_string(),
        max_block_ex_mem: "80000000000000000".to_string(),
        max_block_ex_steps: "18446744073709551615".to_string(),
        max_val_size: 5000,
        collateral_percent: 150.0,
        max_collateral_inputs: 3,
        coins_per_utxo_size: 0,
        min_fee_ref_script_cost_per_byte: 0,
        decentralisation: 0.0,
    }
}

pub fn get_hydra_cost_model() -> Vec<Vec<i64>> {
    get_mainnet_cost_models()
}
