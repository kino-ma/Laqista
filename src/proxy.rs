use std::error::Error;
use std::pin::Pin;

use axum::response::Response;
use futures::Future;

pub async fn create_reverse_proxy(
    package: &str,
    addr: &str,
) -> impl Fn() -> Pin<Box<dyn Future<Output = Result<Response<&'static str>, Box<dyn Error>>>>> {
    let package = package.to_owned();
    let addr = addr.to_owned();
    move || Box::pin(inner_proxy(package.clone(), addr.clone()))
}

async fn inner_proxy(
    package: String,
    addr: String,
) -> Result<Response<&'static str>, Box<dyn Error>> {
    let resp = Response::new("");
    Ok(resp)
}
