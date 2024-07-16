use std::sync::Arc;

use mless_core::server::AbtsractServer;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use crate::proto::{detector_server::Detector, DetectReply, DetectRequest};

type ServerPointer = Arc<Mutex<AbtsractServer<DetectRequest, DetectReply>>>;
pub struct FaceServer(ServerPointer);

#[tonic::async_trait]
impl Detector for FaceServer {
    async fn detect_face(
        &self,
        request: Request<DetectRequest>,
    ) -> Result<Response<DetectReply>, Status> {
        let inner_request = request.into_inner();

        let reply = self
            .0
            .lock()
            .await
            .infer(inner_request)
            .await
            .map_err(|e| Status::aborted(format!("could not run inference: {e}")))?;

        Ok(Response::new(reply))
    }
}
