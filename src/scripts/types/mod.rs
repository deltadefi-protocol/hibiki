pub mod account;
pub use account::*;
use whisky::{data::PlutusDataToJson, ConstrEnum};

#[derive(Debug, Clone, ConstrEnum)]
pub enum MintPolarity {
    RMint,
    RBurn,
}
