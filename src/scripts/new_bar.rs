use std::sync::OnceLock;

static BLUEPRINT_JSON: &str = include_str!("./plutus.json");

pub static BLUEPRINT: OnceLock<Blueprint> = OnceLock::new();

pub fn get_blueprint() -> &'static Blueprint {
    BLUEPRINT.get_or_init(|| {
        serde_json::from_str(BLUEPRINT_JSON).expect("Failed to parse blueprint JSON")
    })
}

use whisky::{
    Blueprint, BuilderDataType, ConstrEnum, ImplConstr, LanguageVersion, MintingBlueprint,
    SpendingBlueprint, WithdrawalBlueprint,
};

use whisky::data::{
    Address, AssetName, Bool, ByteArray, ByteString, Constr0, Constr1, Constr2, Constr3, Constr4,
    Constr5, Constr6, Constr7, Constr8, Constr9, ConstrFields, Credential, Int, List, Map,
    OutputReference, PlutusData, PlutusDataJson, PolicyId, ScriptHash, Tuple, VerificationKeyHash,
};

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
            network_id: 0,
            stake_key_hash: None,
            is_stake_script_credential: false,
        }
    }
}

pub fn app_deposit_withdraw_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, ProcessAppDeposit> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[1].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn app_deposit_publish_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[2].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn app_deposit_request_mint_minting_blueprint(
    params: &PolicyId,
) -> MintingBlueprint<PolicyId, MintPolarity> {
    let script_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(script_config.plutus_version);
    blueprint
        .param_script(
            get_blueprint().validators[4].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn app_deposit_request_spend_spending_blueprint(
    params: &PolicyId,
) -> SpendingBlueprint<PolicyId, AppDepositRequestRedeemer, AppDepositRequestDatum> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    blueprint
        .param_script(
            get_blueprint().validators[6].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn oracle_nft_mint_minting_blueprint(
    params: &OutputReference,
) -> MintingBlueprint<OutputReference, MintPolarity> {
    let script_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(script_config.plutus_version);
    blueprint
        .param_script(
            get_blueprint().validators[8].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn app_oracle_spend_spending_blueprint(
) -> SpendingBlueprint<(), AppOracleRedeemer, AppOracleDatum> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    blueprint
        .no_param_script(get_blueprint().validators[10].compiled_code.as_str())
        .unwrap();
    blueprint
}

pub fn app_vault_stake_rotation_withdraw_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[12].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn app_vault_stake_rotation_publish_withdrawal_blueprint(
    params: &PolicyId,
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

pub fn app_withdrawal_withdraw_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, ProcessAppWithdrawalRedeemer> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[15].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn app_withdrawal_publish_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[16].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn app_vault_spend_spending_blueprint(
    params: &PolicyId,
) -> SpendingBlueprint<PolicyId, AppVaultRedeemer, PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    blueprint
        .param_script(
            get_blueprint().validators[18].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn dex_account_balance_mint_minting_blueprint(
    params: &PolicyId,
) -> MintingBlueprint<PolicyId, MintPolarity> {
    let script_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(script_config.plutus_version);
    blueprint
        .param_script(
            get_blueprint().validators[20].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn dex_account_balance_spend_spending_blueprint(
    params: (&PolicyId, &PolicyId),
) -> SpendingBlueprint<(PolicyId, PolicyId), DexAccountBalanceRedeemer, DexAccountBalanceDatum> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    let param_strs: Vec<String> = vec![params.0.to_json_string(), params.1.to_json_string()];
    let param_refs: Vec<&str> = param_strs.iter().map(|s| s.as_str()).collect();
    blueprint
        .param_script(
            get_blueprint().validators[22].compiled_code.as_str(),
            &param_refs,
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn emergency_order_cancel_withdraw_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, EmergencyCancelRedeemer> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[24].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn emergency_order_cancel_publish_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[25].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn dex_order_book_spend_spending_blueprint(
    params: (&PolicyId, &PolicyId),
) -> SpendingBlueprint<(PolicyId, PolicyId), DexOrderBookRedeemer, DexOrderBookDatum> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    let param_strs: Vec<String> = vec![params.0.to_json_string(), params.1.to_json_string()];
    let param_refs: Vec<&str> = param_strs.iter().map(|s| s.as_str()).collect();
    blueprint
        .param_script(
            get_blueprint().validators[27].compiled_code.as_str(),
            &param_refs,
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn emergency_cancel_order_request_mint_minting_blueprint(
    params: &PolicyId,
) -> MintingBlueprint<PolicyId, MintPolarity> {
    let script_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(script_config.plutus_version);
    blueprint
        .param_script(
            get_blueprint().validators[29].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn emergency_cancel_order_request_spend_spending_blueprint(
    params: &PolicyId,
) -> SpendingBlueprint<PolicyId, EmergencyCancelRequestRedeemer, EmergencyCancelRequestDatum> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    blueprint
        .param_script(
            get_blueprint().validators[31].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn emergency_withdrawal_request_mint_minting_blueprint(
    params: &PolicyId,
) -> MintingBlueprint<PolicyId, MintPolarity> {
    let script_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(script_config.plutus_version);
    blueprint
        .param_script(
            get_blueprint().validators[33].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn emergency_withdrawal_request_spend_spending_blueprint(
    params: &PolicyId,
) -> SpendingBlueprint<PolicyId, EmergencyWithdrawalRequestRedeemer, EmergencyWithdrawalRequestDatum>
{
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    blueprint
        .param_script(
            get_blueprint().validators[35].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn hydra_account_spend_spending_blueprint(
    params: &PolicyId,
) -> SpendingBlueprint<PolicyId, HydraAccountRedeemer, UserAccount> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    blueprint
        .param_script(
            get_blueprint().validators[37].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn hydra_account_withdraw_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, HydraAccountOperation> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[38].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn hydra_order_book_spend_spending_blueprint(
    params: &PolicyId,
) -> SpendingBlueprint<PolicyId, PlutusData, Order> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    blueprint
        .param_script(
            get_blueprint().validators[40].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn hydra_order_book_withdraw_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, HydraOrderBookRedeemer> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[41].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn hydra_order_book_publish_withdrawal_blueprint(
    params: &PolicyId,
) -> WithdrawalBlueprint<PolicyId, PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        WithdrawalBlueprint::new(script_config.plutus_version, script_config.network_id);
    blueprint
        .param_script(
            get_blueprint().validators[42].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn hydra_tokens_mint_minting_blueprint(
    params: &PolicyId,
) -> MintingBlueprint<PolicyId, HydraTokensRedeemer> {
    let script_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(script_config.plutus_version);
    blueprint
        .param_script(
            get_blueprint().validators[44].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn hydra_user_intent_spend_spending_blueprint(
    params: &PolicyId,
) -> SpendingBlueprint<PolicyId, PlutusData, HydraUserIntentDatum> {
    let script_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(script_config.plutus_version, script_config.network_id, None);
    blueprint
        .param_script(
            get_blueprint().validators[46].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn hydra_user_intent_mint_minting_blueprint(
    params: &PolicyId,
) -> MintingBlueprint<PolicyId, HydraUserIntentRedeemer> {
    let script_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(script_config.plutus_version);
    blueprint
        .param_script(
            get_blueprint().validators[47].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn always_succeed_mint_minting_blueprint(
    params: (&HydraAccountIntent, &HydraOrderBookIntent),
) -> MintingBlueprint<(HydraAccountIntent, HydraOrderBookIntent), PlutusData> {
    let script_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(script_config.plutus_version);
    let param_strs: Vec<String> = vec![params.0.to_json_string(), params.1.to_json_string()];
    let param_refs: Vec<&str> = param_strs.iter().map(|s| s.as_str()).collect();
    blueprint
        .param_script(
            get_blueprint().validators[49].compiled_code.as_str(),
            &param_refs,
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

#[derive(Debug, Clone, ImplConstr)]
pub struct ProcessAppDeposit(pub Constr0<MPFProof>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum MPFProof {
    MPFInsert(Proof),
    MPFUpdate(Box<(ByteString, ByteString, Proof)>),
    MPFDelete(Proof),
}

pub type Proof = List<ProofStep>;

#[derive(Debug, Clone, ConstrEnum)]
pub enum ProofStep {
    Branch(Box<(Int, ByteString)>),
    Fork(Box<(Int, Neighbor)>),
    Leaf(Box<(Int, ByteString, ByteString)>),
}

#[derive(Clone, Debug, ImplConstr)]
pub struct Neighbor(pub Constr0<Box<(Int, ByteString, ByteString)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum MintPolarity {
    RMint,
    RBurn,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum AppDepositRequestRedeemer {
    AppDepositRequestTransferAccountBalance,
    AppDepositRequestEmergencyWithdrawal,
    AppDepositRequestSpamPreventionWithdraw,
}

#[derive(Clone, Debug, ImplConstr)]
pub struct AppDepositRequestDatum(pub Constr0<Box<(UserAccount, MValue)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum UserAccount {
    UserTradeAccount(Box<(Account, ScriptHash)>),
    UserFundingAccount(Box<(Account, ScriptHash)>),
    UserMobileAccount(Box<(Account, ScriptHash)>),
}

#[derive(Clone, Debug, ImplConstr)]
pub struct Account(pub Constr0<Box<(ByteString, Credential, Credential)>>);

pub type MValue = Map<PolicyId, Map<AssetName, Int>>;

#[derive(Debug, Clone, ConstrEnum)]
pub enum AppOracleRedeemer {
    DexRotateKey(Box<(ByteString, ByteString)>),
    StopDex,
    RotateHydraInfo(HydraInfo),
    MigrateApp,
}

#[derive(Clone, Debug, ImplConstr)]
pub struct HydraInfo(pub Constr0<List<VerificationKeyHash>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct AppOracleDatum(
    pub  Constr0<
        Box<
            ConstrFields<(
                VerificationKeyHash,
                VerificationKeyHash,
                List<VerificationKeyHash>,
                PolicyId,
                Address,
                ScriptHash,
                PolicyId,
                Address,
                PolicyId,
                Address,
                PolicyId,
                Address,
                PolicyId,
                Address,
                PolicyId,
                Address,
                WithdrawalScriptHashes,
                HydraInfo,
            )>,
        >,
    >,
);

#[derive(Clone, Debug, ImplConstr)]
pub struct WithdrawalScriptHashes(
    pub Constr0<Box<(ScriptHash, ScriptHash, ScriptHash, ScriptHash)>>,
);

#[derive(Debug, Clone, ConstrEnum)]
pub enum ProcessAppWithdrawalRedeemer {
    ProcessAppWithdrawal(Box<(UserAccount, MValue, MPFProof)>),
    CommunityStop,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum AppVaultRedeemer {
    AppVaultWithdraw,
    AppVaultStakeKeyRotation,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum DexAccountBalanceRedeemer {
    AppDeposit,
    AppWithdrawal,
    DABHydraIncrementalDecommit,
    DABHydraCommit,
    HydraWithdrawal,
    HydraCancelWithdrawal,
    DABSplitMerkleTree,
    DABCombineMerkleTree,
    DABSpamPreventionWithdraw,
    DABRemoveRegistry,
}

#[derive(Clone, Debug, ImplConstr)]
pub struct DexAccountBalanceDatum(pub Constr0<ByteString>);

#[derive(Clone, Debug, ImplConstr)]
pub struct EmergencyCancelRedeemer(pub Constr0<Box<(UserAccount, MerklizedOrderDatum, MPFProof)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct MerklizedOrderDatum(pub Constr0<Box<(Order, MValue)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct Order(
    pub  Constr0<
        Box<(
            ByteString,
            Tuple,
            Tuple,
            Bool,
            Int,
            Int,
            Int,
            UserAccount,
            OrderType,
        )>,
    >,
);

#[derive(Debug, Clone, ConstrEnum)]
pub enum OrderType {
    LimitOrder,
    MarketOrder,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum DexOrderBookRedeemer {
    DexOrderBookSplitMerkleTree,
    DexOrderBookCombineMerkleTree,
    DexOrderBookHydraCommit,
    DexOrderBookSpamPreventionWithdraw,
    DexOrderBookEmergencyCancelOrder,
}

#[derive(Clone, Debug, ImplConstr)]
pub struct DexOrderBookDatum(
    pub  Constr0<
        Box<(
            VerificationKeyHash,
            VerificationKeyHash,
            UserAccount,
            ByteString,
            PolicyId,
            Address,
            PolicyId,
            Address,
            ScriptHash,
            ScriptHash,
            ScriptHash,
            PolicyId,
        )>,
    >,
);

#[derive(Debug, Clone, ConstrEnum)]
pub enum EmergencyCancelRequestRedeemer {
    EmergencyRequestProcessCancel,
    EmergencyRequestSpamPreventionCancel,
    EmergencyRequestExpiredCancel,
}

#[derive(Clone, Debug, ImplConstr)]
pub struct EmergencyCancelRequestDatum(pub Constr0<Box<(UserAccount, ByteString, Int)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum EmergencyWithdrawalRequestRedeemer {
    EmergencyRequestProcessEmergencyAction,
    EmergencyRequestSpamPreventionWithdraw,
    EmergencyRequestExpiredWithdraw,
}

#[derive(Clone, Debug, ImplConstr)]
pub struct EmergencyWithdrawalRequestDatum(pub Constr0<Box<(UserAccount, MValue, Int)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraAccountRedeemer {
    HydraAccountTrade(PlutusData),
    HydraAccountOperate,
    HydraAccountSpamPreventionWithdraw,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraAccountOperation {
    ProcessWithdrawal(MPFProof),
    ProcessCancelWithdrawal(MPFProof),
    ProcessSameAccountTransferal(UserAccount),
    ProcessTransferal,
    ProcessCombineUtxosAtClose(TreeOrProofsWithTokenMap),
    ProcessSplitUtxosAtOpen(TreeOrProofsWithTokenMap),
}

#[derive(Clone, Debug, ImplConstr)]
pub struct TreeOrProofsWithTokenMap(pub Constr0<Box<(TreeOrProofs, TokenMap)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum TreeOrProofs {
    FullTree(Tree),
    Proofs(List<MPFProof>),
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum Tree {
    TreeBranch(Box<(ByteString, List<Tree>)>),
    TreeLeaf(Box<(ByteString, ByteString, ByteString)>),
}

pub type TokenMap = Map<ByteString, Tuple>;

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraOrderBookRedeemer {
    PlaceOrder(UserAccount),
    CancelOrder,
    FillOrder(ByteString),
    ModifyOrder(UserAccount),
    CombineOrderMerkle(TreeOrProofsWithTokenMap),
    SplitOrderMerkle(TreeOrProofsWithTokenMap),
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraTokensRedeemer {
    MintAtHydraOpen,
    BurnAtHydraClose,
    MintAtCancelWithdrawal,
    BurnAtWithdrawal,
    MintAtInitOrderBook,
    BurnAtCombineOrderBook,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraUserIntentDatum {
    TradeIntent(Box<(UserAccount, PlutusData)>),
    MasterIntent(Box<(UserAccount, PlutusData)>),
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraUserIntentRedeemer {
    MintTradeIntent(Box<(UserAccount, PlutusData)>),
    MintMasterIntent(Box<(UserAccount, PlutusData)>),
    BurnIntent,
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraAccountIntent {
    WithdrawalIntent(Box<(MValue, MValue)>),
    CancelWithdrawalIntent(MValue),
    TransferIntent(Box<(UserAccount, MValue)>),
}

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraOrderBookIntent {
    PlaceOrderIntent(Box<(Order, MValue)>),
    ModifyOrderIntent(Box<(Order, MValue)>),
}
