pub mod account;
pub mod place_order;
pub use crate::scripts::bar::UserAccount;
pub use place_order::ProtoOrderType;
use whisky::ConstrEnum;

#[derive(Debug, Clone, ConstrEnum)]
pub enum MintPolarity {
    RMint,
    RBurn,
}
