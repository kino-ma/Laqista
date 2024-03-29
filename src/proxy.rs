use std::error::Error;

// use axum::{
//     body::{Body, Bytes, Full},
//     extract::State,
//     http::Request,
//     routing::{any, MethodRouter},
// };
// use hyper::{body::Incoming, client::conn::http2::SendRequest};
// use tokio::net::TcpStream;

use axum::{
    body::{Body, Bytes},
    extract::{Request, State},
    http::uri::Uri,
    response::{IntoResponse, Response},
    routing::{any, get, MethodRouter},
    Router,
};
// use http_body::Body;
use hyper::{body::Incoming, client::conn::http1::SendRequest, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

use hyper_util::client::legacy::connect::HttpConnector;
use tokio::net::TcpStream;

#[derive(Clone, Copy, Debug)]
struct LocalExec;

impl<F> hyper::rt::Executor<F> for LocalExec
where
    F: std::future::Future + 'static, // not requiring `Send`
{
    fn execute(&self, fut: F) {
        // This will spawn into the currently running `LocalSet`.
        tokio::task::spawn_local(fut);
    }
}

pub async fn create_reverse_proxy(
    package: &str,
    addr: &str,
) -> Result<MethodRouter, Box<dyn Error>> {
    let package = package.to_owned();
    let addr = addr.to_owned();

    let stream = TcpStream::connect(addr).await?;
    let mut io = TokioIo::new(stream);
    let mut exec = TokioExecutor::new();
    let (sender, conn) = hyper::client::conn::http2::handshake::<_, _, Body>(exec, io).await?;

    // let client: Client =
    //     hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
    //         .build(HttpConnector::new());

    let handler = any(
        // |State(sender): State<SendRequest<_>>, mut req: Request<_>| async move {
        || async move {
            // let headers = req.headers();
            // let body = req.into_body();

            // let req = hyper::Request::new(body);
            // sender.send_request(req).await;
            ""
        },
    )
    .with_state(sender);

    // panic!()
    Ok(handler)
}
