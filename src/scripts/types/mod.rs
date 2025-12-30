pub mod account;
pub use crate::scripts::bar::UserAccount;
use whisky::ConstrEnum;

#[derive(Debug, Clone, ConstrEnum)]
pub enum MintPolarity {
    RMint,
    RBurn,
}
