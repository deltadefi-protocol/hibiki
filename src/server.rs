use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use whisky::{calculate_tx_hash, Wallet};

use hibiki::{
    config::AppConfig,
    grpc_metrics_interceptor::MetricsLayer,
    handler::{
        cancel_orders, fill_order, internal_transfer, modify_order, place_order,
        process_modify_order, process_order, process_transfer, same_account_transferal,
        serialize_transfer_intent_datum, sign_transaction, sign_transaction_with_fee_collector,
    },
    metrics, metrics_server,
    scripts::ScriptCache,
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
    pub config: Arc<AppConfig>,
    pub scripts: Arc<ScriptCache>,
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

    // Trade
    async fn place_order(
        &self,
        request: Request<services::PlaceOrderRequest>,
    ) -> Result<Response<services::IntentTxResponse>, Status> {
        let request_result = request.into_inner();
        println!("Got a request - place_order {:?}", request_result);

        let reply = match place_order::handler(request_result, &self.config, &self.scripts).await {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn process_order(
        &self,
        request: Request<services::ProcessOrderRequest>,
    ) -> Result<Response<services::ProcessOrderResponse>, Status> {
        let request_result = request.into_inner();
        println!("Got a request - process_order {:?}", request_result);

        let reply = match process_order::handler(
            request_result,
            &self.app_owner_wallet,
            &self.config,
            &self.scripts,
        )
        .await
        {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn cancel_orders(
        &self,
        request: Request<services::CancelOrdersRequest>,
    ) -> Result<Response<services::CancelOrdersResponse>, Status> {
        let request_result = request.into_inner();
        println!("Got a request - cancel_orders {:?}", request_result);

        let reply = match cancel_orders::handler(
            request_result,
            &self.app_owner_wallet,
            &self.config,
            &self.scripts,
        )
        .await
        {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn modify_order(
        &self,
        request: Request<services::ModifyOrderRequest>,
    ) -> Result<Response<services::IntentTxResponse>, Status> {
        let request_result = request.into_inner();
        println!("Got a request - place_order {:?}", request_result);

        let reply = match modify_order::handler(request_result, &self.config, &self.scripts).await {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn process_modify_order(
        &self,
        request: Request<services::ProcessModifyOrderRequest>,
    ) -> Result<Response<services::ProcessModifyOrderResponse>, Status> {
        let request_result = request.into_inner();
        println!("Got a request - process_modify_order {:?}", request_result);

        let reply = match process_modify_order::handler(
            request_result,
            &self.app_owner_wallet,
            &self.config,
            &self.scripts,
        )
        .await
        {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn fill_order(
        &self,
        request: Request<services::FillOrderRequest>,
    ) -> Result<Response<services::FillOrderResponse>, Status> {
        let request_result = request.into_inner();
        println!("Got a request - fill_order {:?}", request_result);

        let reply = match fill_order::handler(
            request_result,
            &self.app_owner_wallet,
            &self.config,
            &self.scripts,
        )
        .await
        {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    // Transfer
    async fn internal_transfer(
        &self,
        request: Request<services::InternalTransferRequest>,
    ) -> Result<Response<services::IntentTxResponse>, Status> {
        let request_result = request.into_inner();
        println!("Got a request - internal_transfer {:?}", request_result);

        let reply =
            match internal_transfer::handler(request_result, &self.config, &self.scripts).await {
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
        let reply = match process_transfer::handler(
            request_result,
            &self.app_owner_wallet,
            &self.config,
            &self.scripts,
        )
        .await
        {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    async fn same_account_transferal(
        &self,
        request: Request<services::SameAccountTransferalRequest>,
    ) -> Result<Response<services::SameAccountTransferalResponse>, Status> {
        let request_result = request.into_inner();
        println!("Got a request - process_transfer {:?}", request_result);
        let reply = match same_account_transferal::handler(
            request_result,
            &self.app_owner_wallet,
            &self.config,
            &self.scripts,
        )
        .await
        {
            Ok(value) => value,
            Err(e) => {
                return Err(Status::failed_precondition(e.to_string()));
            }
        };
        Ok(Response::new(reply))
    }

    // Utils
    async fn serialize_transferal_intent_datum(
        &self,
        request: Request<services::SerializeTransferalIntentDatumRequest>,
    ) -> Result<Response<services::SerializeDatumResponse>, Status> {
        println!("Got a request - serialize_transferal_intent_datum");
        let request_result = request.into_inner();
        let reply = match serialize_transfer_intent_datum::handler(request_result, &self.scripts) {
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

    // Initialize config and scripts once at startup
    let config = Arc::new(AppConfig::new());
    let scripts = Arc::new(ScriptCache::new());

    let transactions = HibikiService {
        app_owner_wallet: Arc::new(get_app_owner_wallet()),
        fee_collector_wallet: Arc::new(get_fee_collector_wallet()),
        config,
        scripts,
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
