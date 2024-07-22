use std::{error::Error, sync::Arc};

use mless_core::{server::AbtsractServer, session::Session};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use crate::{
    model_path,
    proto::{detector_server::Detector, InferReply, InferRequest},
};

type ServerPointer = Arc<Mutex<AbtsractServer<InferRequest, InferReply>>>;
pub struct FaceServer(ServerPointer);

impl FaceServer {
    pub async fn create() -> Result<Self, Box<dyn Error>> {
        let path = model_path();
        let session = Session::from_path(path).await?;
        let server = AbtsractServer::new(session);
        let ptr = Arc::new(Mutex::new(server));

        Ok(Self(ptr))
    }
}

#[tonic::async_trait]
impl Detector for FaceServer {
    async fn infer(&self, request: Request<InferRequest>) -> Result<Response<InferReply>, Status> {
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
