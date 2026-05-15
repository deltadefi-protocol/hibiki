use hibiki_proto::services::{IntentTxResponse, ScriptAccountInternalTransferRequest};
use whisky::{
    calculate_tx_hash,
    data::{ByteString, PlutusDataJson, PolicyId},
    script_hash_to_stake_address, Asset, Budget, WData, WError, WRedeemer,
};

use crate::{
    config::AppConfig,
    scripts::{
        vault_withdraw_withdrawal_blueprint, HydraAccountIntent, HydraUserIntentDatum,
        HydraUserIntentRedeemer, ScriptCache, UserAccount,
    },
    utils::{
        hydra::get_hydra_tx_builder,
        proto::{assets_to_mvalue, from_proto_amount, from_proto_utxo},
        token::to_hydra_token,
    },
};

pub async fn handler(
    request: ScriptAccountInternalTransferRequest,
    config: &AppConfig,
    scripts: &ScriptCache,
) -> Result<IntentTxResponse, WError> {
    let app_owner_vkey = &config.app_owner_vkey;
    let network_id = config.network_id.parse::<u8>().unwrap_or(0);
    let account_ops_script_hash = &scripts.hydra_order_book_withdrawal.hash;

    let collateral = from_proto_utxo(request.collateral_utxo.as_ref().unwrap());
    let empty_utxo = from_proto_utxo(request.empty_utxo.as_ref().unwrap());
    let ref_input = from_proto_utxo(request.dex_order_book_utxo.as_ref().unwrap());
    let vault_oracle_utxo = from_proto_utxo(request.vault_oracle_utxo.as_ref().unwrap());

    let vault_account_info = request.vault_account.unwrap();
    let owner_account_info = request.owner_account.unwrap();

    let mut tx_builder = get_hydra_tx_builder(true);
    let user_intent_mint = &scripts.user_intent_mint;
    let user_intent_spend = &scripts.user_intent_spend;

    let from_account =
        UserAccount::from_proto_trade_account(&vault_account_info, account_ops_script_hash);
    let to_account = UserAccount::from_proto_trade_account(
        &request.receiver_account.unwrap(),
        account_ops_script_hash,
    );
    let transfer_amount_l2 =
        assets_to_mvalue(&to_hydra_token(&from_proto_amount(&request.to_transfer)));

    // Create transfer intent
    let hydra_account_intent =
        HydraAccountIntent::TransferIntent(Box::new((to_account.clone(), transfer_amount_l2)));
    let intent = Box::new((from_account, hydra_account_intent));

    // Vault withdrawal script: parametrised by the vault oracle NFT unit found in the oracle utxo
    let vault_oracle_nft = vault_oracle_utxo
        .output
        .amount
        .iter()
        .find(|asset| asset.unit() != "lovelace")
        .map(|asset| asset.unit())
        .ok_or_else(|| {
            WError::new(
                "script_account_internal_transfer",
                "vault_oracle_utxo is missing the vault oracle NFT",
            )
        })?;
    let vault_withdrawal = vault_withdraw_withdrawal_blueprint(PolicyId::new(&vault_oracle_nft));
    let vault_withdrawal_stake_address =
        script_hash_to_stake_address(&vault_account_info.master_key, network_id)?;
    let vault_withdrawal_redeemer = WRedeemer {
        data: WData::JSON(ByteString::new("").to_json_string()),
        ex_units: Budget::default(),
    };

    tx_builder
        .read_only_tx_in_reference(&ref_input.input.tx_hash, ref_input.input.output_index, None)
        .input_for_evaluation(&ref_input)
        // vault oracle utxo as read-only reference
        .read_only_tx_in_reference(
            &vault_oracle_utxo.input.tx_hash,
            vault_oracle_utxo.input.output_index,
            None,
        )
        .input_for_evaluation(&vault_oracle_utxo)
        .mint_plutus_script_v3()
        .mint(1, &user_intent_mint.hash, "")
        .mint_redeemer_value(&user_intent_mint.redeemer(
            HydraUserIntentRedeemer::MintMasterIntent(intent.clone()),
            None,
        ))
        .mint_tx_in_reference(
            &collateral.input.tx_hash,
            user_intent_mint.ref_output_index,
            &user_intent_mint.hash,
            user_intent_mint.size,
        )
        .input_for_evaluation(&user_intent_mint.ref_utxo(&collateral)?)
        .tx_out(
            &user_intent_spend.address,
            &[Asset::new_from_str(&user_intent_mint.hash, "1")],
        )
        .tx_out_inline_datum_value(&WData::JSON(
            HydraUserIntentDatum::MasterIntent(intent).to_json_string(),
        ))
        .tx_in(
            &empty_utxo.input.tx_hash,
            empty_utxo.input.output_index,
            &empty_utxo.output.amount,
            &empty_utxo.output.address,
        )
        .input_for_evaluation(&empty_utxo)
        .tx_out(&empty_utxo.output.address, &empty_utxo.output.amount)
        // vault withdrawal authorises spending from the script account
        .withdrawal_plutus_script_v3()
        .withdrawal(&vault_withdrawal_stake_address, 0)
        .withdrawal_script(&vault_withdrawal.cbor)
        .withdrawal_redeemer_value(&vault_withdrawal_redeemer)
        .required_signer_hash(app_owner_vkey)
        .required_signer_hash(&owner_account_info.master_key)
        .tx_in_collateral(
            &collateral.input.tx_hash,
            collateral.input.output_index,
            &collateral.output.amount,
            &collateral.output.address,
        )
        .input_for_evaluation(&collateral)
        .change_address(&request.address);

    log::debug!(
        "[SCRIPT_ACCOUNT_INTERNAL_TRANSFER] tx_builder.mint_item: {:?}",
        tx_builder.mint_item
    );

    tx_builder.complete(None).await?;

    let tx_hex = tx_builder.tx_hex();
    let tx_hash = calculate_tx_hash(&tx_hex)?;

    Ok(IntentTxResponse {
        tx_hex,
        tx_hash,
        tx_index: 0,
        new_empty_utxo_tx_index: 1,
    })
}
