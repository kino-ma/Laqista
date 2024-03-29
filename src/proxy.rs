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
    body::Body,
    extract::{Request, State},
    routing::{any, MethodRouter},
};
// use http_body::Body;
use hyper::client::conn::http2::SendRequest;
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
    let _package = package.to_owned();
    let addr = addr.to_owned();

    let stream = TcpStream::connect(addr).await?;
    let io = TokioIo::new(stream);
    let exec = TokioExecutor::new();
    let (sender, _conn) = hyper::client::conn::http2::handshake::<_, _, Body>(exec, io).await?;

    let handler = any(
        |State(mut sender): State<SendRequest<_>>, req: Request| async move {
            let headers = req.headers().clone();
            let body = req.into_body();

            let mut req = hyper::Request::new(body);
            req.headers_mut().clone_from(&headers);
            sender
                .send_request(req)
                .await
                .map_err(|e| println!("failed to send request to destination: {}", e))
                .unwrap()
        },
    )
    .with_state(sender);

    // panic!()
    Ok(handler)
}
