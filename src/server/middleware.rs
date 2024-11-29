use axum::async_trait;
use tokio::time::Instant;
use tonic::{
    body::BoxBody,
    codegen::http::{Request, Response},
    transport::Body,
};
use tonic_middleware::{Middleware, ServiceBound};

use super::AppMetricSender;

pub struct MetricsMiddleware {
    tx: AppMetricSender,
}

#[async_trait]
impl<S> Middleware<S> for MetricsMiddleware
where
    S: ServiceBound,
    S::Future: Send,
{
    async fn call(
        &self,
        req: Request<Body>,
        mut service: S,
    ) -> Result<Response<BoxBody>, S::Error> {
        let start_time = Instant::now();
        // Call the service. You can also intercept request from middleware.
        let result = service.call(req).await?;

        let elapsed_time = start_time.elapsed();
        println!("Request processed in {:?}", elapsed_time);

        Ok(result)
    }
}
