use std::error::Error;

use axum::{
    body::{Body, Bytes, Full},
    extract::State,
    http::Request,
    routing::{any, MethodRouter},
};
use hyper::{body::Incoming, client::conn::http2::SendRequest};
use tokio::net::TcpStream;

use hyper_util::rt::{TokioExecutor, TokioIo};

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
) -> Result<MethodRouter<()>, Box<dyn Error>> {
    let package = package.to_owned();
    let addr = addr.to_owned();

    let stream = TcpStream::connect(addr).await?;
    let mut io = TokioIo::new(stream);
    let mut exec = TokioExecutor::new();
    // let (mut sender, conn) = hyper::client::conn::http2::handshake(&mut exec, io).await?;
    let conn = hyper::client::conn::http2::handshake(exec, io);
    let (mut sender, conn) = conn.await?;

    // let incoming1: Incoming = panic!();
    // let x: Request<Incoming> = Request::new(incoming1);
    // let incoming = x.into_body();

    sender.send_request(hyper::Request::new(Bytes::new()));

    // let handler = any(
    //     |State(_): State<SendRequest<_>>, mut req: Request<Incoming>| async move {
    //         let headers = req.headers();
    //         let body = req.into_body();

    //         let req = hyper::Request::new(body);
    //         sender.send_request(req).await;
    //         ""
    //     },
    // )
    // .with_state(sender);

    panic!()
}
