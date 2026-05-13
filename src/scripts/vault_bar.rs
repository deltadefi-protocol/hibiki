use std::sync::OnceLock;

static BLUEPRINT_JSON: &str = include_str!("./starka-plutus.json");

pub static BLUEPRINT: OnceLock<Blueprint> = OnceLock::new();

pub fn get_blueprint() -> &'static Blueprint {
    BLUEPRINT.get_or_init(|| {
        serde_json::from_str(BLUEPRINT_JSON).expect("Failed to parse blueprint JSON")
    })
}

use whisky::{
    Blueprint, BuilderDataType, ConstrEnum, LanguageVersion, MintingBlueprint, SpendingBlueprint,
    WithdrawalBlueprint,
};

use whisky::data::{OutputReference, PlutusData, PlutusDataJson, PolicyId};

use crate::config::AppConfig;

pub struct ScriptConfig {
    pub plutus_version: LanguageVersion,
    pub network_id: u8,
    pub stake_key_hash: Option<String>,
    pub is_stake_script_credential: bool,
}

impl ScriptConfig {
    pub fn new() -> Self {
        Self {
            plutus_version: LanguageVersion::V3,
            network_id: AppConfig::new().network_id.parse().unwrap(),
            stake_key_hash: None,
            is_stake_script_credential: false,
        }
    }
}

pub fn vault_spend_spending_blueprint(
    params: PolicyId,
) -> SpendingBlueprint<PolicyId, VaultSpendRedeemer, PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    blueprint
        .param_script(
            get_blueprint().validators[12].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn vault_withdraw_withdrawal_blueprint(
    params: PolicyId,
) -> WithdrawalBlueprint<PolicyId, PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[13].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn vault_oracle_spend_spending_blueprint(
    params: (OutputReference, PolicyId),
) -> SpendingBlueprint<(OutputReference, PolicyId), VaultOracleSpendRedeemer, PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    let param_strs: Vec<String> = vec![params.0.to_json_string(), params.1.to_json_string()];
    let param_refs: Vec<&str> = param_strs.iter().map(|s| s.as_str()).collect();
    blueprint
        .param_script(
            get_blueprint().validators[18].compiled_code.as_str(),
            &param_refs,
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn vault_oracle_mint_minting_blueprint(
    params: (OutputReference, PolicyId),
) -> MintingBlueprint<(OutputReference, PolicyId), VaultOracleMintRedeemer> {
    let script_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(script_config.plutus_version);
    let param_strs: Vec<String> = vec![params.0.to_json_string(), params.1.to_json_string()];
    let param_refs: Vec<&str> = param_strs.iter().map(|s| s.as_str()).collect();
    blueprint
        .param_script(
            get_blueprint().validators[19].compiled_code.as_str(),
            &param_refs,
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum VaultSpendRedeemer {
    ProcessWithdrawal,
    DepositIntoDeltaDeFi,
    StakeRotation,
    VaultPluggableLogic,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum VaultOracleSpendRedeemer {
    ProcessL1Deposit,
    ProcessL1Withdrawal,
    ProcessL2Deposit,
    ProcessL2Withdrawal,
    HydraCommit,
    HydraDecommit,
    UpdateConfig,
    BurnVault,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum VaultOracleMintRedeemer {
    MintVault,
    CloseVault,
}
