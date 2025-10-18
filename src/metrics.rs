use lazy_static::lazy_static;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder,
};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // gRPC Request metrics
    pub static ref GRPC_REQUEST_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "hibiki_grpc_request_duration_seconds",
            "Duration of gRPC requests in seconds"
        )
        .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0]),
        &["method", "status"]
    )
    .expect("Failed to create GRPC_REQUEST_DURATION metric");

    pub static ref GRPC_REQUESTS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "hibiki_grpc_requests_total",
            "Total number of gRPC requests"
        ),
        &["method", "status"]
    )
    .expect("Failed to create GRPC_REQUESTS_TOTAL metric");
}

/// Initialize the Prometheus registry with all metrics
pub fn init_metrics() {
    REGISTRY
        .register(Box::new(GRPC_REQUEST_DURATION.clone()))
        .expect("Failed to register GRPC_REQUEST_DURATION");

    REGISTRY
        .register(Box::new(GRPC_REQUESTS_TOTAL.clone()))
        .expect("Failed to register GRPC_REQUESTS_TOTAL");
}

/// Gather metrics and encode them in Prometheus text format
pub fn gather_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
