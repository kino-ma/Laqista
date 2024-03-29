use std::error::Error;
use std::pin::Pin;

use axum::response::Response;
use futures::Future;

pub async fn create_reverse_proxy(
    package: &str,
    addr: &str,
) -> impl Fn() -> Pin<Box<dyn Future<Output = Result<Response<&'static str>, Box<dyn Error>>>>> {
    || Box::pin(inner_proxy())
}

async fn inner_proxy() -> Result<Response<&'static str>, Box<dyn Error>> {
    let resp = Response::new("");
    Ok(resp)
}
