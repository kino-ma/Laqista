use mless_core::server::AbtsractServer;
use tonic::{Request, Response, Status};

use crate::proto::{detector_server::Detector, DetectReply, DetectRequest};

pub struct FaceServer(AbtsractServer<DetectRequest, DetectReply>);

#[tonic::async_trait]
impl Detector for FaceServer {
    async fn detect_face(
        &self,
        request: Request<DetectRequest>,
    ) -> Result<Response<DetectReply>, Status> {
        let out = self
            .0
            .infer(input)
            .await
            .map_err(|e| Status::aborted(format!("could not run inference: {e}")))?;
        Ok(())
    }
}
