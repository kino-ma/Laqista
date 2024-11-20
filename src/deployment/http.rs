use bytes::Bytes;
use reqwest::Result as ReqResult;

pub async fn download(url: String) -> ReqResult<Bytes> {
    reqwest::get(url).await?.bytes().await
}
