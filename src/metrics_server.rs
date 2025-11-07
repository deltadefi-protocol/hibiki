use hyper::{
    header::CONTENT_TYPE,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server, StatusCode,
};
use std::convert::Infallible;
use std::net::SocketAddr;

use crate::metrics;

async fn metrics_handler(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let metrics_output = metrics::gather_metrics();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/plain; version=0.0.4")
        .body(Body::from(metrics_output))
        .unwrap())
}

async fn health_handler(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let response = serde_json::json!({
        "status": "ok",
        "service": "hibiki",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(response.to_string()))
        .unwrap())
}

async fn router(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    match req.uri().path() {
        "/metrics" => metrics_handler(req).await,
        "/health" => health_handler(req).await,
        "/" => health_handler(req).await,
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap()),
    }
}

pub async fn start_metrics_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(router)) });

    let server = Server::bind(&addr).serve(make_svc);

    println!("Metrics server listening on http://{}", addr);
    println!("Metrics endpoint: http://{}/metrics", addr);
    println!("Health endpoint: http://{}/health", addr);

    server.await?;

    Ok(())
}
