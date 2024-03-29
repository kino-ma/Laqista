use std::error::Error;

use axum::{
    body::{BoxBody, Bytes},
    extract::State,
    handler::Handler,
    response::IntoResponse,
    routing::{any, MethodRouter},
};
use bytes::Buf;
use h2::client::{self, SendRequest};
use http_body::{combinators::UnsyncBoxBody, Body};
use hyper::{HeaderMap, Request};
use tokio::net::TcpStream;

pub async fn create_reverse_proxy(
    package: &str,
    addr: &str,
) -> Result<MethodRouter<SendRequest<Bytes>, UnsyncBoxBody<Bytes, ()>>, Box<dyn Error>> {
    // let _package = package.to_owned();
    // let addr = addr.to_owned();

    // let stream = TcpStream::connect(addr).await?;
    // let io = TokioIo::new(stream);
    // let exec = TokioExecutor::new();
    // let (sender, _conn) = hyper::client::conn::http2::handshake::<_, _, Body>(exec, io).await?;

    let tcp = TcpStream::connect(addr).await?;
    let (mut client, h2) = client::handshake(tcp).await?;

    let b = Bytes::new();
    b.remaining();

    let handler = any(
        |State(mut sender): State<SendRequest<Bytes>>, req: Request<UnsyncBoxBody<_, _>>| async move {
            let headers = req.headers().clone();
            let uri = req.uri().clone();
            let body = req.into_body().collect().await.expect("failed to collect data").to_bytes();

            let mut req = Request::builder()
                .uri(uri)
                .body(())
                .expect(&format!("failed to send request to destination"));

            req.headers_mut().clone_from(&headers);

            let (resp, mut stream) = sender
                .send_request(req, false)
                .map_err(|e| println!("failed to send request to destination: {}", e))
                .unwrap();

            stream.send_data(body, true);

            let resp = resp.await.expect("failed to receive the response");

            let body = resp.into_body().data().await.unwrap().unwrap();

            body.as_ref().to_owned()
        },
    )
    .with_state(client);

    // panic!()
    Ok(handler)
}
