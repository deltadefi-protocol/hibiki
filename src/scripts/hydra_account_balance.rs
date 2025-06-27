use super::types::UserAccount;
use crate::config::AppConfig;
use whisky::data::Value;
use whisky::ConstrEnum;
use whisky::{data::PlutusDataToJson, utils::blueprint::SpendingBlueprint, LanguageVersion};

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraAccountBalanceRedeemer {
    UpdateBalanceWithPlaceOrder,
    UpdataBalanceWithFillOrder,
    UpdataBalanceWithCancelOrder,
    UpdataBalanceWithReleaseExtraValue,
    UpdateBalanceWithWithdrawal,
    UpdateBalanceWithCancelWithdrawal,
    UpdateBalanceWithTransfer,
    HydraCombineUtxosAtClose,
    HydraAccountBalanceRemoveEmptyBalance,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraAccountBalanceDatum {
    Datum(UserAccount, Value),
}

pub fn hydra_account_balance_spending_blueprint(
) -> SpendingBlueprint<(), HydraAccountBalanceRedeemer, HydraAccountBalanceDatum> {
    let AppConfig { network_id, .. } = AppConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(LanguageVersion::V3, network_id.parse().unwrap(), None);
    blueprint
      .no_param_script(
          "58b658b40101009800aba2a6011e581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c00a6010847392d7370656e640048c8c8c8c88c88966002646464646464660020026eb0c038c03cc03cc03cc03cc03cc03cc03cc03cc030dd5180718061baa0072259800800c52844c96600266e3cdd71808001005c528c4cc00c00c00500d1808000a01c300c300d002300b001300b002300900130063754003149a26cac8028dd7000ab9a5573caae7d5d09",
      )
      .unwrap();
    blueprint
}

#[cfg(test)]
mod tests {

    use super::*;
    use dotenv::dotenv;

    #[test]
    fn test_hydra_account_balance_spending_blueprint() {
        dotenv().ok();

        let blueprint = hydra_account_balance_spending_blueprint();
        assert_eq!(
            blueprint.hash,
            "1318e21d5eb0eb93f23a4d9a52592db44dd48e971fe6de91a8c14071"
        );
        assert_eq!(
          blueprint.cbor,
          "58b658b40101009800aba2a6011e581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c00a6010847392d7370656e640048c8c8c8c88c88966002646464646464660020026eb0c038c03cc03cc03cc03cc03cc03cc03cc03cc030dd5180718061baa0072259800800c52844c96600266e3cdd71808001005c528c4cc00c00c00500d1808000a01c300c300d002300b001300b002300900130063754003149a26cac8028dd7000ab9a5573caae7d5d09"
      );
    }
}
