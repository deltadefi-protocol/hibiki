use whisky::data::{ByteString, Constr0, Constr1, Constr2, Credential, PolicyId, ScriptHash};

use crate::{
    constant::{dex_oracle_nft, SCRIPTS},
    scripts::{
        bar::{Account, MintMasterIntent, TransferIntent, UserTradeAccount},
        MValue, MasterIntent,
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
        let blueprint = (SCRIPTS.hydra_order_book.withdrawal)(&policy_id);
        let script_hash = ScriptHash::new(&blueprint.hash);

        match account_info.account_type.as_str() {
            "spot_account" => UserTradeAccount(Constr0::new(Box::new((account, script_hash)))),
            _ => panic!("Unknown account type: {}", account_info.account_type),
        }
    }
}

impl TransferIntent {
    pub fn new(account: UserTradeAccount, value: MValue) -> TransferIntent {
        TransferIntent(Constr2::new(Box::new((account, value))))
    }
}

impl MintMasterIntent {
    pub fn new(account: UserTradeAccount, intent: TransferIntent) -> MintMasterIntent {
        MintMasterIntent(Constr1::new(Box::new((account, intent))))
    }
}

impl MasterIntent {
    pub fn new(account: UserTradeAccount, intent: TransferIntent) -> MasterIntent {
        MasterIntent(Constr1::new(Box::new((account, intent))))
    }
}
