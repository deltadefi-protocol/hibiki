use std::task::{Context, Poll};
use tower::{Layer, Service};
use std::time::Instant;

use crate::metrics;

/// Layer that adds metrics tracking to all gRPC requests
#[derive(Clone)]
pub struct MetricsLayer;

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;

    fn layer(&self, service: S) -> Self::Service {
        MetricsService { inner: service }
    }
}

/// Service that wraps all gRPC calls with metrics tracking
#[derive(Clone)]
pub struct MetricsService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for MetricsService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = futures::future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        // Extract the gRPC method name from the URI path
        // gRPC paths look like: /package.Service/Method
        let method_name = req
            .uri()
            .path()
            .split('/')
            .last()
            .unwrap_or("unknown")
            .to_string();

        let start = Instant::now();
        let mut inner = self.inner.clone();
        let future = inner.call(req);

        Box::pin(async move {
            let response = future.await?;

            // Determine status from HTTP status code
            let status = if response.status().is_success() {
                "ok"
            } else {
                "error"
            };

            // Record metrics
            let duration = start.elapsed().as_secs_f64();
            metrics::GRPC_REQUEST_DURATION
                .with_label_values(&[&method_name, status])
                .observe(duration);

            metrics::GRPC_REQUESTS_TOTAL
                .with_label_values(&[&method_name, status])
                .inc();

            Ok(response)
        })
    }
}
