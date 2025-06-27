use whisky::{UTxO, UtxoInput, UtxoOutput};

use crate::utils::proto::from_proto_amount;

pub fn from_proto_utxo(proto: &hibiki_proto::services::UTxO) -> UTxO {
    let proto_input = proto.input.as_ref().unwrap();
    let proto_output = proto.output.as_ref().unwrap();
    let data_hash = (!proto_output.data_hash.is_empty()).then(|| proto_output.data_hash.clone());
    let plutus_data =
        (!proto_output.plutus_data.is_empty()).then(|| proto_output.plutus_data.clone());
    let script_ref = (!proto_output.script_ref.is_empty()).then(|| proto_output.script_ref.clone());
    let script_hash =
        (!proto_output.script_hash.is_empty()).then(|| proto_output.script_hash.clone());

    UTxO {
        input: UtxoInput {
            output_index: proto_input.output_index,
            tx_hash: proto_input.tx_hash.clone(),
        },
        output: UtxoOutput {
            address: proto_output.address.clone(),
            amount: from_proto_amount(&proto_output.amount),
            data_hash,
            plutus_data,
            script_ref,
            script_hash,
        },
    }
}
