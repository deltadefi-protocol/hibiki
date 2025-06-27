use super::types::UserAccount;
use crate::config::AppConfig;
use whisky::ConstrEnum;
use whisky::{
    data::{Bool, ByteString, Constr0, Int, PlutusDataToJson, Tuple},
    utils::blueprint::{MintingBlueprint, SpendingBlueprint},
    LanguageVersion,
};

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraUserIntentRedeemer {
    MintPlaceOrderIntent(
        Constr0<
            Box<(
                ByteString,
                Tuple<(ByteString, ByteString)>,
                Tuple<(ByteString, ByteString)>,
                Bool,
                Int,
                Int,
                Int,
                Int,
                UserAccount,
            )>,
        >,
    ),
    HydraUserPlaceOrder,
    MintCancelOrderIntent,
    HydraUserCancelOrder,
    MintWithdrawalIntent,
    HydraUserWithdrawal,
    MintTransferIntent,
    HydraUserTransfer,
    BurnIntent,
}

pub fn hydra_user_intent_spending_blueprint(
) -> SpendingBlueprint<ByteString, ByteString, ByteString> {
    let AppConfig { network_id, .. } = AppConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(LanguageVersion::V3, network_id.parse().unwrap(), None);
    blueprint
      .no_param_script(
          "58b658b40101009800aba2a6011e581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c00a6010847382d7370656e640048c8c8c8c88c88966002646464646464660020026eb0c038c03cc03cc03cc03cc03cc03cc03cc03cc030dd5180718061baa0072259800800c52844c96600266e3cdd71808001005c528c4cc00c00c00500d1808000a01c300c300d002300b001300b002300900130063754003149a26cac8028dd7000ab9a5573caae7d5d09",
      )
      .unwrap();
    blueprint
}

pub fn hydra_user_intent_minting_blueprint() -> MintingBlueprint<(), HydraUserIntentRedeemer> {
    let mut blueprint = MintingBlueprint::new(LanguageVersion::V3);
    blueprint
      .no_param_script(
          "58b558b30101009800aba2a6011e581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c00a6010746382d6d696e740048c8c8c8c88c88966002646464646464660020026eb0c038c03cc03cc03cc03cc03cc03cc03cc03cc030dd5180718061baa0072259800800c52844c96600266e3cdd71808001005c528c4cc00c00c00500d1808000a01c300c300d002300b001300b002300900130063754003149a26cac8028dd7000ab9a5573caae7d5d09",
      )
      .unwrap();
    blueprint
}

#[cfg(test)]
mod tests {

    use super::*;
    use dotenv::dotenv;

    #[test]
    fn test_hydra_user_intent_spending_blueprint() {
        dotenv().ok();

        let blueprint = hydra_user_intent_spending_blueprint();
        assert_eq!(
            blueprint.hash,
            "eb0a5938244e92fd172560f530bf959724b10353a26f276ea8bbb3cc"
        );
        assert_eq!(
          blueprint.cbor,
          "58b658b40101009800aba2a6011e581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c00a6010847382d7370656e640048c8c8c8c88c88966002646464646464660020026eb0c038c03cc03cc03cc03cc03cc03cc03cc03cc030dd5180718061baa0072259800800c52844c96600266e3cdd71808001005c528c4cc00c00c00500d1808000a01c300c300d002300b001300b002300900130063754003149a26cac8028dd7000ab9a5573caae7d5d09"
      );
    }

    #[test]
    fn test_hydra_user_intent_minting_blueprint() {
        dotenv().ok();

        let blueprint = hydra_user_intent_minting_blueprint();
        assert_eq!(
            blueprint.hash,
            "463e70d04718e253757523698184cb7090b0430e89dc025c4c8e392c"
        );
        assert_eq!(
          blueprint.cbor,
          "58b558b30101009800aba2a6011e581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c00a6010746382d6d696e740048c8c8c8c88c88966002646464646464660020026eb0c038c03cc03cc03cc03cc03cc03cc03cc03cc030dd5180718061baa0072259800800c52844c96600266e3cdd71808001005c528c4cc00c00c00500d1808000a01c300c300d002300b001300b002300900130063754003149a26cac8028dd7000ab9a5573caae7d5d09"
      );
    }
}
