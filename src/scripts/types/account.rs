use whisky::data::{ByteString, Constr0, Constr1, Constr2, Credential, PolicyId, ScriptHash};

use crate::{
    constant::{dex_oracle_nft, SCRIPTS},
    scripts::{
        bar::{
            Account, CancelWithdrawalIntent, HydraAccountIntent, HydraOrderBookRedeemer,
            HydraUserIntentRedeemer, MValue, MintMasterIntent, TransferIntent, UserAccount,
            UserFundingAccount, UserMobileAccount, UserTradeAccount, WithdrawalIntent,
        },
        MasterIntent,
    },
};

impl Account {
    pub fn new_from_keys(
        account_id: ByteString,
        master_key: (&str, bool),
        operation_key: (&str, bool),
    ) -> Self {
        let master_credential = Credential::new(master_key);
        let operation_credential = Credential::new(operation_key);

        Account(Constr0::new(Box::new((
            account_id,
            master_credential,
            operation_credential,
        ))))
    }
}

impl UserTradeAccount {
    pub fn new_from_account<P>(
        account: Account,
        hydra_order_book_withdrawal_blueprint: fn(
            PolicyId,
        ) -> whisky::WithdrawalBlueprint<
            P,
            HydraOrderBookRedeemer,
        >,
    ) -> UserAccount
    where
        P: whisky::data::PlutusDataJson,
    {
        let policy_id = PolicyId::new(dex_oracle_nft());
        let blueprint = hydra_order_book_withdrawal_blueprint(policy_id);
        let script_hash = ScriptHash::new(&blueprint.hash);

        UserAccount::UserTradeAccount(UserTradeAccount(Constr0::new(Box::new((
            account,
            script_hash,
        )))))
    }
}

impl UserAccount {
    pub fn from_proto(account_info: &hibiki_proto::services::AccountInfo) -> Self {
        let clean_account_id = account_info.account_id.replace("-", "");

        let account = Account::new_from_keys(
            ByteString::new(&clean_account_id),
            (&account_info.master_key, account_info.is_script_master_key),
            (
                &account_info.operation_key,
                account_info.is_script_operation_key,
            ),
        );

        let policy_id = PolicyId::new(dex_oracle_nft());
        let blueprint = (SCRIPTS.hydra_order_book.withdrawal)(policy_id);
        let script_hash = ScriptHash::new(&blueprint.hash);

        match account_info.account_type.as_str() {
            "spot_account" => UserAccount::UserTradeAccount(UserTradeAccount(Constr0::new(
                Box::new((account, script_hash)),
            ))),
            "funding_account" => UserAccount::UserFundingAccount(UserFundingAccount(Constr1::new(
                Box::new((account, script_hash)),
            ))),
            "mobile_account" => UserAccount::UserMobileAccount(UserMobileAccount(Constr2::new(
                Box::new((account, script_hash)),
            ))),
            _ => panic!("Unknown account type: {}", account_info.account_type),
        }
    }
}

impl WithdrawalIntent {
    pub fn new(value: MValue) -> HydraAccountIntent {
        HydraAccountIntent::WithdrawalIntent(WithdrawalIntent(Constr0::new(value)))
    }
}

impl CancelWithdrawalIntent {
    pub fn new(value: MValue) -> HydraAccountIntent {
        HydraAccountIntent::CancelWithdrawalIntent(CancelWithdrawalIntent(Constr1::new(value)))
    }
}

impl TransferIntent {
    pub fn new(account: UserAccount, value: MValue) -> HydraAccountIntent {
        HydraAccountIntent::TransferIntent(TransferIntent(Constr2::new(Box::new((account, value)))))
    }
}

impl HydraAccountIntent {
    pub fn withdrawal(value: MValue) -> Self {
        WithdrawalIntent::new(value)
    }

    pub fn cancel_withdrawal(value: MValue) -> Self {
        CancelWithdrawalIntent::new(value)
    }

    pub fn transfer(account: UserAccount, value: MValue) -> Self {
        TransferIntent::new(account, value)
    }
}

impl MintMasterIntent {
    pub fn new(account: UserAccount, intent: HydraAccountIntent) -> HydraUserIntentRedeemer {
        HydraUserIntentRedeemer::MintMasterIntent(MintMasterIntent(Constr1::new(Box::new((
            account, intent,
        )))))
    }
}

impl MasterIntent {
    pub fn new(account: UserAccount, intent: HydraAccountIntent) -> MasterIntent {
        MasterIntent(Constr1::new(Box::new((account, intent))))
    }
}
