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
