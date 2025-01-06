use std::{error::Error, sync::Arc};

use bytes::Bytes;
use laqista_core::{
    proto::host::HostCall, server::AbtsractServer, session::Session, wasm::WasmRunner,
};
use tonic::{Request, Response, Status};
use wasmer::Value;

use crate::proto::{
    detector_server::Detector, object_detection_server::ObjectDetection, DetectionReply,
    DetectionRequest, InferReply, InferRequest,
};

type ServerPointer = Arc<AbtsractServer<InferRequest, InferReply>>;
pub struct FaceServer {
    inner: ServerPointer,
}

impl FaceServer {
    pub async fn create(onnx: Bytes, wasm: Bytes) -> Result<Self, Box<dyn Error>> {
        let session = Session::from_bytes(&onnx).await?;
        let module = WasmRunner::compile(&wasm)?;
        let server = AbtsractServer::new(session, module, onnx.clone(), wasm.clone());
        let ptr = Arc::new(server);

        Ok(Self { inner: ptr })
    }
}

#[tonic::async_trait]
impl Detector for FaceServer {
    async fn run_detection(
        &self,
        request: Request<DetectionRequest>,
    ) -> Result<Response<DetectionReply>, Status> {
        let mut wasm = self
            .inner
            .get_module()
            .instantiate()
            .map_err(|e| Status::aborted(format!("failed to instantiate wasm module: {e}")))?;

        let msg = request.into_inner();
        let ptr = wasm.write_message(msg).map_err(|e| {
            Status::aborted(format!("Failed to write request data to wasm memory: {e}"))
        })?;

        let params: &[Value; 2] = &ptr.into();

        let call: HostCall = wasm
            .call::<()>("main", params)
            .map_err(|e| Status::aborted(format!("Failed to call WebAssembly function: {e}")))?
            .unwrap_continue();

        let param_ptr = call
            .parameters
            .ok_or(Status::aborted("Failed to read HostCall: cont is null"))?;

        let params: InferRequest = wasm.read_message(param_ptr.into()).map_err(|e| {
            Status::aborted(format!(
                "Failed to read infer parameters from wasm memory: {e}"
            ))
        })?;

        let req = Request::new(params);
        let resp = self.squeeze(req).await?.into_inner();

        let ptr = wasm.write_message(resp).map_err(|e| {
            Status::aborted(format!("Failed to write infer reply to wasm memory: {e}"))
        })?;
        let param: [Value; 2] = ptr.into();

        let cont = call
            .cont
            .ok_or(Status::aborted("Failed to read HostCall: cont is null"))?;

        let reply: DetectionReply = wasm
            .call(&cont.name, &param)
            .map_err(|e| Status::aborted(format!("Failed to call WebAssembly function (2): {e}")))?
            .unwrap_finished();

        Ok(Response::new(reply))
    }
}

#[tonic::async_trait]
impl ObjectDetection for FaceServer {
    async fn squeeze(
        &self,
        request: Request<InferRequest>,
    ) -> Result<Response<InferReply>, Status> {
        let inner_request = request.into_inner();

        let reply = self
            .inner
            .infer(inner_request)
            .await
            .map_err(|e| Status::aborted(format!("could not run inference: {e}")))?;

        Ok(Response::new(reply))
    }
}
