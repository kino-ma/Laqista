use std::error::Error;

use axum::routing::{any, MethodRouter};
use h2::client;
use http_body::Body;
use hyper::Request;
use tokio::net::TcpStream;

pub async fn create_reverse_proxy(
    package: &str,
    addr: &str,
) -> Result<MethodRouter, Box<dyn Error>> {
    let _package = package.to_owned();
    let addr = addr.to_owned();

    let handler: MethodRouter = any(|req: Request<hyper::body::Body>| async move {
        let tcp = TcpStream::connect(addr)
            .await
            .expect("failed to connect the server");

        let (mut client, _h2) = client::handshake(tcp)
            .await
            .expect("failed to handshake with the server");

        let headers = req.headers().clone();
        let uri = req.uri().clone();
        let body = req
            .into_body()
            .collect()
            .await
            .expect("failed to collect data")
            .to_bytes();

        let mut req = Request::builder()
            .uri(uri)
            .body(())
            .expect(&format!("failed to send request to destination"));

        req.headers_mut().clone_from(&headers);

        let (resp, mut stream) = client
            .send_request(req, false)
            .map_err(|e| println!("failed to send request to destination: {}", e))
            .unwrap();

        stream.send_data(body, true).expect("failed to send data");

        let resp = resp.await.expect("failed to receive the response");

        let body = resp.into_body().data().await.unwrap().unwrap();

        body.as_ref().to_owned()
    });

    // panic!()
    Ok(handler)
}
