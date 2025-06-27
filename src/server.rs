use dotenv::dotenv;
use std::env;
use whisky::calculate_tx_hash;

use hibiki::{
    handler::{
        create_hydra_account_utxo, internal_transfer, process_transfer,
        serialize_transfer_intent_datum, sign_transaction,
    },
    services::{
        self,
        hibiki_server::{Hibiki, HibikiServer},
        TxHashResponse,
    },
};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug, Default)]
pub struct HibikiService {}

#[tonic::async_trait]
impl Hibiki for HibikiService {
    async fn ping_hello(
        &self,
        _request: Request<services::HelloRequest>,
    ) -> Result<Response<services::HelloResponse>, Status> {
        let reply = services::HelloResponse {
            message: "Hello from Hibiki!".to_string(),
        };
        Ok(Response::new(reply))
    }

    async fn internal_transfer(
        &self,
        request: Request<services::InternalTransferRequest>,
    ) -> Result<Response<services::IntentTxResponse>, Status> {
        println!("Got a request - internal_transfer");
        let request_result = request.into_inner();

        let reply = match internal_transfer::handler(request_result) {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn process_transfer(
        &self,
        request: Request<services::ProcessTransferRequest>,
    ) -> Result<Response<services::ProcessTransferResponse>, Status> {
        println!("Got a request - process_transfer");
        let request_result = request.into_inner();
        let reply = match process_transfer::handler(request_result) {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn create_hydra_account_utxo(
        &self,
        request: Request<services::CreateHydraAccountUtxoRequest>,
    ) -> Result<Response<services::CreateHydraAccountUtxoResponse>, Status> {
        println!("Got a request - create_hydra_account_utxo");
        let request_result = request.into_inner();
        let reply = match create_hydra_account_utxo::handler(request_result) {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn serialize_transferal_intent_datum(
        &self,
        request: Request<services::SerializeTransferalIntentDatumRequest>,
    ) -> Result<Response<services::SerializeDatumResponse>, Status> {
        println!("Got a request - serialize_transferal_intent_datum");
        let request_result = request.into_inner();
        let reply = match serialize_transfer_intent_datum::handler(request_result) {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn sign_transaction(
        &self,
        request: Request<services::SignTransactionRequest>,
    ) -> Result<Response<services::SignTransactionResponse>, Status> {
        println!("Got a request - sign_transaction");
        let request_result = request.into_inner();
        let reply = match sign_transaction::handler(request_result) {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn calculate_tx_hash(
        &self,
        request: Request<services::CalculateTxHashRequest>,
    ) -> Result<Response<services::TxHashResponse>, Status> {
        println!("Got a request - calculate_tx_hash");
        let request_result = request.into_inner();
        let tx_hash = match calculate_tx_hash(&request_result.tx_hex) {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(TxHashResponse { tx_hash: tx_hash }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let port = env::var("PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("127.0.0.1:{}", port).parse()?;
    let transactions = HibikiService::default();

    println!("Server listening on port {}...", port);
    Server::builder()
        .add_service(HibikiServer::new(transactions))
        .serve(addr)
        .await?;
    Ok(())
}
