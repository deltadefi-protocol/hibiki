use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use whisky::{calculate_tx_hash, Wallet};

use hibiki::{
    grpc_metrics_interceptor::MetricsLayer,
    handler::{
        create_hydra_account_utxo, internal_transfer, process_transfer,
        serialize_transfer_intent_datum, sign_transaction, sign_transaction_with_fee_collector,
    },
    metrics, metrics_server,
    services::{
        self,
        hibiki_server::{Hibiki, HibikiServer},
        TxHashResponse,
    },
    utils::wallet::{get_app_owner_wallet, get_fee_collector_wallet},
};
use std::time::Instant;
use tonic::{transport::Server, Request, Response, Status};

pub struct HibikiService {
    pub app_owner_wallet: Arc<Wallet>,
    pub fee_collector_wallet: Arc<Wallet>,
}

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
        let request_result = request.into_inner();
        println!("Got a request - internal_transfer {:?}", request_result);

        let reply = match internal_transfer::handler(request_result).await {
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
        let request_result = request.into_inner();
        println!("Got a request - process_transfer {:?}", request_result);
        let reply = match process_transfer::handler(request_result, &self.app_owner_wallet).await {
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
        let request_result = request.into_inner();
        println!(
            "Got a request - create_hydra_account_utxo {:?}",
            request_result
        );
        let reply = match create_hydra_account_utxo::handler(request_result, &self.app_owner_wallet)
            .await
        {
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
        let start = Instant::now();
        println!("Got a request - sign_transaction");
        let request_result = request.into_inner();
        let reply = match sign_transaction::handler(request_result, &self.app_owner_wallet) {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        println!("Time taken for sign_transaction: {:?}", start.elapsed());
        Ok(Response::new(reply))
    }

    async fn sign_transaction_with_fee_collector(
        &self,
        request: Request<services::SignTransactionRequest>,
    ) -> Result<Response<services::SignTransactionResponse>, Status> {
        let start = Instant::now();
        println!("Got a request - sign_transaction_with_fee_collector");
        let request_result = request.into_inner();
        let reply = match sign_transaction_with_fee_collector::handler(
            request_result,
            &self.fee_collector_wallet,
        ) {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        println!(
            "Time taken for sign_transaction_with_fee_collector: {:?}",
            start.elapsed()
        );
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

    // Initialize Prometheus metrics
    metrics::init_metrics();

    let grpc_port = env::var("PORT").unwrap_or_else(|_| "50051".to_string());
    let metrics_port: u16 = env::var("METRICS_PORT")
        .unwrap_or_else(|_| "9090".to_string())
        .parse()
        .expect("METRICS_PORT must be a valid port number");

    let grpc_addr = format!("0.0.0.0:{}", grpc_port).parse()?;
    let transactions = HibikiService {
        app_owner_wallet: Arc::new(get_app_owner_wallet()),
        fee_collector_wallet: Arc::new(get_fee_collector_wallet()),
    };

    println!("gRPC Server listening on port {}...", grpc_port);
    println!("Metrics server will listen on port {}...", metrics_port);

    // Start metrics server in background
    tokio::spawn(async move {
        if let Err(e) = metrics_server::start_metrics_server(metrics_port).await {
            eprintln!("Metrics server error: {}", e);
        }
    });

    // Start gRPC server with metrics layer
    Server::builder()
        .layer(MetricsLayer)
        .add_service(HibikiServer::new(transactions))
        .serve(grpc_addr)
        .await?;

    Ok(())
}
