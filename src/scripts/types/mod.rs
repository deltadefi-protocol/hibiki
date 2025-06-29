pub mod account;
pub use account::*;
use whisky::ConstrEnum;

#[derive(Debug, Clone, ConstrEnum)]
pub enum MintPolarity {
    RMint,
    RBurn,
}
