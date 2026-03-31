use crate::scripts::bar::{Account, UserAccount};
use whisky::data::{ByteString, Constr0, Credential, ScriptHash};

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

impl UserAccount {
    pub fn from_proto_trade_account(
        account_info: &hibiki_proto::services::AccountInfo,
        account_ops_script_hash: &str,
    ) -> Self {
        let clean_account_id = account_info.account_id.replace("-", "");

        let account = Account::new_from_keys(
            ByteString::new(&clean_account_id),
            (&account_info.master_key, account_info.is_script_master_key),
            (
                &account_info.operation_key,
                account_info.is_script_operation_key,
            ),
        );
        let script_hash = ScriptHash::new(account_ops_script_hash);
        match account_info.account_type.as_str() {
            "spot_account" => UserAccount::UserTradeAccount(Box::new((account, script_hash))),
            _ => panic!("Unknown account type: {}", account_info.account_type),
        }
    }
}
