use axum::async_trait;
use tokio::time::Instant;
use tonic::{
    body::BoxBody,
    codegen::http::{Request, Response},
    transport::Body,
};
use tonic_middleware::{Middleware, ServiceBound};

use super::{AppMetric, AppMetricSender};

#[derive(Clone)]
pub struct MetricsMiddleware {
    pub tx: AppMetricSender,
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
        let path = req.uri().path().to_owned();

        let [_, app_svc, rpc] = path.split("/").collect::<Vec<_>>()[..] else {
            panic!("invalid path: {path:?}")
        };
        let [app, svc] = app_svc.split(".").collect::<Vec<_>>()[..] else {
            panic!("invalid path: {path:?}")
        };

        let start_time = Instant::now();
        // Call the service. You can also intercept request from middleware.
        let result = service.call(req).await?;

        let elapsed = start_time.elapsed();

        let metric = AppMetric {
            app: app.to_owned(),
            service: svc.to_owned(),
            rpc: rpc.to_owned(),
            elapsed,
        };

        self.tx.send(metric).await.unwrap();

        Ok(result)
    }
}
