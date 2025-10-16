use whisky::{
    data::byte_string, utils::blueprint::WithdrawalBlueprint, BuilderDataType, LanguageVersion,
};

use crate::config::AppConfig;

pub fn hydra_internal_transfer_blueprint() -> WithdrawalBlueprint {
    let AppConfig {
        network_id,
        dex_oracle_nft,
        ..
    } = AppConfig::new();

    let mut blueprint = WithdrawalBlueprint::new(LanguageVersion::V3, network_id.parse().unwrap());
    blueprint
    .param_script(
        "58c958c70101009800aba2a6011e581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c00a6011b5819312d68796472612d696e7465726e616c2d7472616e736665720048c8c8c8c88c88966002646464646464660020026eb0c038c03cc03cc03cc03cc03cc03cc03cc03cc030dd5180718061baa0072259800800c52844c96600266e3cdd71808001005c528c4cc00c00c00500d1808000a01c300c300d002300b001300b002300900130063754003149a26cac8028dd7000ab9a5573caae7d5d09",
&[&byte_string(&dex_oracle_nft).to_string(), ],BuilderDataType::JSON,)
    .unwrap();
    blueprint
}

#[cfg(test)]
mod tests {

    use super::*;
    use dotenv::dotenv;

    #[test]
    fn test_hydra_internal_transfer_blueprint() {
        dotenv().ok();

        let blueprint = hydra_internal_transfer_blueprint();
        assert_eq!(
            blueprint.hash,
            "4442f121ae5abdc9d801053ebaed381cabd12ec8b251f19f4e4d1271"
        );
        assert_eq!(
          blueprint.cbor,
          "58c958c70101009800aba2a6011e581cfa5136e9e9ecbc9071da73eeb6c9a4ff73cbf436105cf8380d1c525c00a6011b5819312d68796472612d696e7465726e616c2d7472616e736665720048c8c8c8c88c88966002646464646464660020026eb0c038c03cc03cc03cc03cc03cc03cc03cc03cc030dd5180718061baa0072259800800c52844c96600266e3cdd71808001005c528c4cc00c00c00500d1808000a01c300c300d002300b001300b002300900130063754003149a26cac8028dd7000ab9a5573caae7d5d09"
      );
    }
}
