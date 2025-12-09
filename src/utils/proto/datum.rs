use whisky::{Asset, UTxO, WError};

/// Extracts the transfer amount from a transfer intent UTXO's plutus datum
///
/// Structure: MasterIntent(sender_account, TransferIntent(receiver_account, transfer_amount))
pub fn extract_transfer_amount_from_intent(intent_utxo: &UTxO) -> Result<Vec<Asset>, WError> {
    use whisky::csl;

    let plutus_data_hex = intent_utxo
        .output
        .plutus_data
        .as_ref()
        .ok_or_else(|| WError::new("Missing plutus data in intent UTXO", "InvalidDataError"))?;

    // Decode plutus data from hex
    let plutus_data = csl::PlutusData::from_hex(plutus_data_hex)
        .map_err(|_| WError::new("Failed to decode intent plutus data", "InvalidDataError"))?;

    // Decode to JSON
    let datum_json = csl::decode_plutus_datum_to_json_value(
        &plutus_data,
        csl::PlutusDatumSchema::DetailedSchema,
    )
    .map_err(|_| WError::new("Failed to decode intent datum to JSON", "InvalidDatumError"))?;

    // Extract fields: MasterIntent = Constr1(sender_account, HydraAccountIntent)
    let master_intent_fields = datum_json["fields"]
        .as_array()
        .ok_or_else(|| WError::new("Invalid MasterIntent structure", "InvalidDataError"))?;

    // Extract HydraAccountIntent (second field)
    let hydra_account_intent = &master_intent_fields[1];

    // Extract TransferIntent fields: TransferIntent = Constr2(receiver_account, transfer_amount)
    let transfer_intent_fields = hydra_account_intent["fields"]
        .as_array()
        .ok_or_else(|| WError::new("Invalid TransferIntent structure", "InvalidDataError"))?;

    // Extract transfer amount (second field in TransferIntent)
    let transfer_amount_map = &transfer_intent_fields[1]["map"];

    // Convert MValue (map) to Vec<Asset>
    let assets = parse_mvalue_to_assets(transfer_amount_map)?;

    Ok(assets)
}

/// Parses a Cardano MValue (plutus map structure) to a vector of Assets
pub fn parse_mvalue_to_assets(mvalue_map: &serde_json::Value) -> Result<Vec<Asset>, WError> {
    let mut assets = Vec::new();

    let map_array = mvalue_map
        .as_array()
        .ok_or_else(|| WError::new("Invalid MValue map structure", "InvalidDataError"))?;

    for entry in map_array {
        let policy_id = entry["k"]["bytes"]
            .as_str()
            .ok_or_else(|| WError::new("Missing policy ID in MValue", "InvalidDataError"))?;

        let inner_map = entry["v"]["map"]
            .as_array()
            .ok_or_else(|| WError::new("Invalid inner map in MValue", "InvalidDataError"))?;

        for token_entry in inner_map {
            let token_name = token_entry["k"]["bytes"]
                .as_str()
                .ok_or_else(|| WError::new("Missing token name in MValue", "InvalidDataError"))?;

            let quantity = token_entry["v"]["int"]
                .as_i64()
                .ok_or_else(|| WError::new("Missing quantity in MValue", "InvalidDataError"))?;

            // Construct unit: policy_id + token_name
            let unit = if token_name.is_empty() {
                policy_id.to_string()
            } else {
                format!("{}{}", policy_id, token_name)
            };

            assets.push(Asset::new(unit, quantity.to_string()));
        }
    }

    Ok(assets)
}
