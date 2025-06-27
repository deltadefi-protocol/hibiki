use serde_json::Value;
use whisky::{
    data::{ByteString, Constr0, Credential, PlutusDataToJson},
    impl_constr_wrapper_type,
};
use whisky::{ConstrEnum, ConstrWrapper};

#[derive(Debug, Clone, ConstrEnum)]
pub enum UserAccount {
    UserSpotAccount(Account),
    UserFundingAccount(Account),
    UserMobileAccount(Account),
}

#[derive(Debug, Clone, ConstrWrapper)]
pub struct Account(Constr0<Box<(ByteString, Credential, Credential)>>);
impl_constr_wrapper_type!(Account, 0, [
  (account_id: ByteString, &str),
  (master_key: Credential, (&str, bool)),
  (operation_key: Credential, (&str, bool)),
]);

impl UserAccount {
    pub fn from_proto(account_info: hibiki_proto::services::AccountInfo) -> Self {
        let account = Account::from(
            &account_info.account_id,
            (&account_info.master_key, account_info.is_script_master_key),
            (
                &account_info.operation_key,
                account_info.is_script_operation_key,
            ),
        );
        match account_info.account_type.as_str() {
            "spot_account" => UserAccount::UserSpotAccount(account),
            "funding_account" => UserAccount::UserFundingAccount(account),
            "mobile_account" => UserAccount::UserMobileAccount(account),
            _ => panic!("Unknown account type"),
        }
    }
}
