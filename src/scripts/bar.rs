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

pub fn oracle_nft_mint_minting_blueprint(
    params: OutputReference,
) -> MintingBlueprint<OutputReference, MintPolarity> {
    let app_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(app_config.plutus_version);
    blueprint
        .param_script(
            get_blueprint().validators[8].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

pub fn dex_order_book_spend_spending_blueprint(
    params: (PolicyId, PolicyId),
) -> SpendingBlueprint<(PolicyId, PolicyId), DexOrderBookRedeemer, DexOrderBookDatum> {
    let app_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(app_config.plutus_version, app_config.network_id, None);
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

pub fn hydra_account_spend_spending_blueprint(
    params: &PolicyId,
) -> SpendingBlueprint<PolicyId, HydraAccountRedeemer, UserAccount> {
    let app_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(app_config.plutus_version, app_config.network_id, None);
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
    let app_config = ScriptConfig::new();
    let mut blueprint = WithdrawalBlueprint::new(app_config.plutus_version, app_config.network_id);
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
    let app_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(app_config.plutus_version, app_config.network_id, None);
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
    let app_config = ScriptConfig::new();
    let mut blueprint = WithdrawalBlueprint::new(app_config.plutus_version, app_config.network_id);
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
    let app_config = ScriptConfig::new();
    let mut blueprint = WithdrawalBlueprint::new(app_config.plutus_version, app_config.network_id);
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
    let app_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(app_config.plutus_version);
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
    let app_config = ScriptConfig::new();
    let mut blueprint =
        SpendingBlueprint::new(app_config.plutus_version, app_config.network_id, None);
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
    let app_config = ScriptConfig::new();
    let mut blueprint = MintingBlueprint::new(app_config.plutus_version);
    blueprint
        .param_script(
            get_blueprint().validators[47].compiled_code.as_str(),
            &[&params.to_json_string()],
            BuilderDataType::JSON,
        )
        .unwrap();
    blueprint
}

#[derive(Debug, Clone, ImplConstr)]
pub struct ProcessAppDeposit(pub Constr0<Box<List<MPFProof>>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum MPFProof {
    MPFInsert(MPFInsert),
    MPFUpdate(MPFUpdate),
    MPFDelete(MPFDelete),
}

#[derive(Debug, Clone, ImplConstr)]
pub struct MPFInsert(pub Constr0<Box<List<ProofStep>>>);

pub type Proof = List<ProofStep>;

#[derive(Debug, Clone, ConstrEnum)]
pub enum ProofStep {
    Branch(Branch),
    Fork(Fork),
    Leaf(Leaf),
}

#[derive(Clone, Debug, ImplConstr)]
pub struct Branch(pub Constr0<Box<(Int, ByteArray)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct Fork(pub Constr1<Box<(Int, Neighbor)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct Neighbor(pub Constr0<Box<(Int, ByteArray, ByteArray)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct Leaf(pub Constr2<Box<(Int, ByteArray, ByteArray)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct MPFUpdate(pub Constr1<Box<(ByteArray, ByteArray, Proof)>>);

#[derive(Debug, Clone, ImplConstr)]
pub struct MPFDelete(pub Constr2<Box<List<ProofStep>>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum MintPolarity {
    RMint,
    RBurn,
}

pub type RMint = Constr0<()>;

pub type RBurn = Constr1<()>;

#[derive(Debug, Clone, ConstrEnum)]
pub enum AppDepositRequestRedeemer {
    AppDepositRequestTransferAccountBalance,
    AppDepositRequestEmergencyWithdrawal,
    AppDepositRequestSpamPreventionWithdraw,
}

pub type AppDepositRequestTransferAccountBalance = Constr0<()>;

pub type AppDepositRequestEmergencyWithdrawal = Constr1<()>;

pub type AppDepositRequestSpamPreventionWithdraw = Constr2<()>;

#[derive(Clone, Debug, ImplConstr)]
pub struct AppDepositRequestDatum(pub Constr0<Box<(UserAccount, MValue)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum UserAccount {
    UserTradeAccount(UserTradeAccount),
    UserFundingAccount(UserFundingAccount),
    UserMobileAccount(UserMobileAccount),
}

#[derive(Clone, Debug, ImplConstr)]
pub struct UserTradeAccount(pub Constr0<Box<(Account, ScriptHash)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct Account(pub Constr0<Box<(ByteArray, Credential, Credential)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct UserFundingAccount(pub Constr1<Box<(Account, ScriptHash)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct UserMobileAccount(pub Constr2<Box<(Account, ScriptHash)>>);

pub type MValue = Map<PolicyId, Map<AssetName, Int>>;

#[derive(Debug, Clone, ConstrEnum)]
pub enum AppOracleRedeemer {
    DexRotateKey(DexRotateKey),
    StopDex,
    RotateHydraInfo(RotateHydraInfo),
    RotateCommunityStopKeys(RotateCommunityStopKeys),
}

#[derive(Clone, Debug, ImplConstr)]
pub struct DexRotateKey(pub Constr0<Box<(ByteArray, ByteArray)>>);

pub type StopDex = Constr1<()>;

#[derive(Clone, Debug, ImplConstr)]
pub struct RotateHydraInfo(pub Constr2<Box<HydraInfo>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct HydraInfo(pub Constr0<Box<(ScriptHash, List<VerificationKeyHash>)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct RotateCommunityStopKeys(pub Constr3<Box<List<VerificationKeyHash>>>);

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
                Address,
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
pub struct WithdrawalScriptHashes(pub Constr0<Box<(ScriptHash, ScriptHash, ScriptHash)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum ProcessAppWithdrawalRedeemer {
    ProcessAppWithdrawal(ProcessAppWithdrawal),
    CommunityStop,
}

#[derive(Clone, Debug, ImplConstr)]
pub struct ProcessAppWithdrawal(pub Constr0<Box<(UserAccount, MValue, MPFProof)>>);

pub type CommunityStop = Constr1<()>;

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

pub type AppDeposit = Constr0<()>;

pub type AppWithdrawal = Constr1<()>;

pub type DABHydraIncrementalDecommit = Constr2<()>;

pub type DABHydraCommit = Constr3<()>;

pub type HydraWithdrawal = Constr4<()>;

pub type HydraCancelWithdrawal = Constr5<()>;

pub type DABSplitMerkleTree = Constr6<()>;

pub type DABCombineMerkleTree = Constr7<()>;

pub type DABSpamPreventionWithdraw = Constr8<()>;

pub type DABRemoveRegistry = Constr9<()>;

#[derive(Clone, Debug, ImplConstr)]
pub struct DexAccountBalanceDatum(pub Constr0<Box<ByteArray>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct EmergencyCancelRedeemer(pub Constr0<Box<(UserAccount, MerklizedOrderDatum, MPFProof)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct MerklizedOrderDatum(pub Constr0<Box<(Order, MValue)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct Order(pub Constr0<Box<(ByteArray, Tuple, Tuple, Bool, Int, Int, Int, UserAccount)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum DexOrderBookRedeemer {
    DexOrderBookSplitMerkleTree,
    DexOrderBookCombineMerkleTree,
    DexOrderBookHydraCommit,
    DexOrderBookSpamPreventionWithdraw,
    DexOrderBookEmergencyCancelOrder,
}

pub type DexOrderBookSplitMerkleTree = Constr0<()>;

pub type DexOrderBookCombineMerkleTree = Constr1<()>;

pub type DexOrderBookHydraCommit = Constr2<()>;

pub type DexOrderBookSpamPreventionWithdraw = Constr3<()>;

pub type DexOrderBookEmergencyCancelOrder = Constr4<()>;

#[derive(Clone, Debug, ImplConstr)]
pub struct DexOrderBookDatum(
    pub  Constr0<
        Box<(
            VerificationKeyHash,
            VerificationKeyHash,
            UserAccount,
            ByteArray,
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

pub type EmergencyRequestProcessCancel = Constr0<()>;

pub type EmergencyRequestSpamPreventionCancel = Constr1<()>;

pub type EmergencyRequestExpiredCancel = Constr2<()>;

#[derive(Clone, Debug, ImplConstr)]
pub struct EmergencyCancelRequestDatum(pub Constr0<Box<(UserAccount, ByteArray, Int)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum EmergencyWithdrawalRequestRedeemer {
    EmergencyRequestProcessEmergencyAction,
    EmergencyRequestSpamPreventionWithdraw,
    EmergencyRequestExpiredWithdraw,
}

pub type EmergencyRequestProcessEmergencyAction = Constr0<()>;

pub type EmergencyRequestSpamPreventionWithdraw = Constr1<()>;

pub type EmergencyRequestExpiredWithdraw = Constr2<()>;

#[derive(Clone, Debug, ImplConstr)]
pub struct EmergencyWithdrawalRequestDatum(pub Constr0<Box<(UserAccount, MValue, Int)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraAccountRedeemer {
    HydraAccountTrade(HydraAccountTrade),
    HydraAccountOperate,
    HydraAccountSpamPreventionWithdraw,
}

#[derive(Clone, Debug, ImplConstr)]
pub struct HydraAccountTrade(pub Constr0<Box<PlutusData>>);

pub type HydraAccountOperate = Constr1<()>;

pub type HydraAccountSpamPreventionWithdraw = Constr2<()>;

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraAccountOperation {
    ProcessWithdrawal(ProcessWithdrawal),
    ProcessCancelWithdrawal(ProcessCancelWithdrawal),
    ProcessSameAccountTransferal(ProcessSameAccountTransferal),
    ProcessTransferal,
    ProcessCombineUtxosAtClose(ProcessCombineUtxosAtClose),
    ProcessSplitUtxosAtOpen(ProcessSplitUtxosAtOpen),
}

#[derive(Debug, Clone, ImplConstr)]
pub struct ProcessWithdrawal(pub Constr0<Box<List<MPFProof>>>);

#[derive(Debug, Clone, ImplConstr)]
pub struct ProcessCancelWithdrawal(pub Constr1<Box<List<MPFProof>>>);

#[derive(Debug, Clone, ImplConstr)]
pub struct ProcessSameAccountTransferal(pub Constr2<Box<List<UserAccount>>>);

pub type ProcessTransferal = Constr3<()>;

#[derive(Debug, Clone, ImplConstr)]
pub struct ProcessCombineUtxosAtClose(pub Constr4<Box<List<TreeOrProofsWithTokenMap>>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct TreeOrProofsWithTokenMap(pub Constr0<Box<(TreeOrProofs, TokenMap)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum TreeOrProofs {
    FullTree(FullTree),
    Proofs(Proofs),
}

#[derive(Debug, Clone, ImplConstr)]
pub struct FullTree(pub Constr0<Box<List<Tree>>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum Tree {
    TreeBranch(TreeBranch),
    TreeLeaf(TreeLeaf),
}

#[derive(Clone, Debug, ImplConstr)]
pub struct TreeBranch(pub Constr0<Box<(ByteArray, PlutusData)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct TreeLeaf(pub Constr1<Box<(ByteArray, ByteArray, ByteArray)>>);

#[derive(Debug, Clone, ImplConstr)]
pub struct Proofs(pub Constr1<Box<List<MPFProof>>>);

pub type TokenMap = Map<ByteString, Tuple>;

#[derive(Debug, Clone, ImplConstr)]
pub struct ProcessSplitUtxosAtOpen(pub Constr5<Box<List<TreeOrProofsWithTokenMap>>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraOrderBookRedeemer {
    PlaceOrder(PlaceOrder),
    CancelOrder,
    FillOrder(FillOrder),
    ModifyOrder(ModifyOrder),
    CombineOrderMerkle(CombineOrderMerkle),
    SplitOrderMerkle(SplitOrderMerkle),
}

#[derive(Debug, Clone, ImplConstr)]
pub struct PlaceOrder(pub Constr0<Box<List<UserAccount>>>);

pub type CancelOrder = Constr1<()>;

#[derive(Clone, Debug, ImplConstr)]
pub struct FillOrder(pub Constr2<Box<ByteArray>>);

#[derive(Debug, Clone, ImplConstr)]
pub struct ModifyOrder(pub Constr3<Box<List<UserAccount>>>);

#[derive(Debug, Clone, ImplConstr)]
pub struct CombineOrderMerkle(pub Constr4<Box<List<TreeOrProofsWithTokenMap>>>);

#[derive(Debug, Clone, ImplConstr)]
pub struct SplitOrderMerkle(pub Constr5<Box<List<TreeOrProofsWithTokenMap>>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraTokensRedeemer {
    MintAtHydraOpen,
    BurnAtHydraClose,
    MintAtCancelWithdrawal,
    BurnAtWithdrawal,
    MintAtInitOrderBook,
    BurnAtCombineOrderBook,
}

pub type MintAtHydraOpen = Constr0<()>;

pub type BurnAtHydraClose = Constr1<()>;

pub type MintAtCancelWithdrawal = Constr2<()>;

pub type BurnAtWithdrawal = Constr3<()>;

pub type MintAtInitOrderBook = Constr4<()>;

pub type BurnAtCombineOrderBook = Constr5<()>;

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraUserIntentDatum {
    TradeIntent(TradeIntent),
    MasterIntent(MasterIntent),
}

#[derive(Clone, Debug, ImplConstr)]
pub struct TradeIntent(pub Constr0<Box<(UserAccount, PlutusData)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct MasterIntent(pub Constr1<Box<(UserTradeAccount, TransferIntent)>>);

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraUserIntentRedeemer {
    MintTradeIntent(MintTradeIntent),
    MintMasterIntent(MintMasterIntent),
    BurnIntent,
}

#[derive(Clone, Debug, ImplConstr)]
pub struct MintTradeIntent(pub Constr0<Box<(UserAccount, PlutusData)>>);

#[derive(Clone, Debug, ImplConstr)]
pub struct MintMasterIntent(pub Constr1<Box<(UserTradeAccount, TransferIntent)>>);

pub type BurnIntent = Constr2<()>;

#[derive(Debug, Clone, ConstrEnum)]
pub enum HydraAccountIntent {
    WithdrawalIntent(WithdrawalIntent),
    CancelWithdrawalIntent(CancelWithdrawalIntent),
    TransferIntent(TransferIntent),
}

#[derive(Clone, Debug, ImplConstr)]
pub struct WithdrawalIntent(pub Constr0<MValue>);

#[derive(Clone, Debug, ImplConstr)]
pub struct CancelWithdrawalIntent(pub Constr1<MValue>);

#[derive(Clone, Debug, ImplConstr)]
pub struct TransferIntent(pub Constr2<Box<(UserTradeAccount, MValue)>>);
