use std::{error::Error, sync::Arc};

use mless_core::{
    proto::host::HostCall, server::AbtsractServer, session::Session, wasm::WasmRunner,
};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use wasmer::Value;

use crate::{
    model_path,
    proto::{
        detector_server::Detector, DetectionReply, DetectionRequest, InferReply, InferRequest,
    },
};

type ServerPointer = Arc<Mutex<AbtsractServer<InferRequest, InferReply>>>;
pub struct FaceServer {
    inner: ServerPointer,
}

static WASM: &'static [u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/face_wasm.wasm");

impl FaceServer {
    pub async fn create() -> Result<Self, Box<dyn Error>> {
        let path = model_path();
        let session = Session::from_path(path).await?;
        let server = AbtsractServer::new(session);
        let ptr = Arc::new(Mutex::new(server));

        Ok(Self { inner: ptr })
    }
}

#[tonic::async_trait]
impl Detector for FaceServer {
    async fn infer(&self, request: Request<InferRequest>) -> Result<Response<InferReply>, Status> {
        let inner_request = request.into_inner();

        let reply = self
            .inner
            .lock()
            .await
            .infer(inner_request)
            .await
            .map_err(|e| Status::aborted(format!("could not run inference: {e}")))?;

        Ok(Response::new(reply))
    }

    async fn run_detection(
        &self,
        request: Request<DetectionRequest>,
    ) -> Result<Response<DetectionReply>, Status> {
        // FIXME: It would be better performant if we could instantiate the wasm module only once.
        //        However, if we reuse the instance for every request, it errors out with message
        //        saying "failed to allocate memory".
        //        Instead, we instantiate the module from compiler, for each request.
        let mut wasm = WasmRunner::compile(WASM).map_err(|e| {
            Status::aborted(format!("Failed to compile and setup wasm module: {e}"))
        })?;

        println!("Imports: {:?}", &wasm.module.imports().collect::<Vec<_>>());
        println!("Exports: {:?}", &wasm.instance.exports);

        let msg = request.into_inner();
        let ptr = wasm.write_message(msg).map_err(|e| {
            Status::aborted(format!("Failed to write request data to wasm memory: {e}"))
        })?;

        let params: &[Value; 2] = &ptr.into();

        let call: HostCall = wasm
            .call::<()>("main", params)
            .map_err(|e| Status::aborted(format!("Failed to call WebAssembly function: {e}")))?
            .unwrap_continue();

        println!("HastCall invoked: {:?}", call);

        let param_ptr = call
            .parameters
            .ok_or(Status::aborted("Failed to read HostCall: cont is null"))?;

        let params: InferRequest = wasm.read_message(param_ptr.into()).map_err(|e| {
            Status::aborted(format!(
                "Failed to read infer parameters from wasm memory: {e}"
            ))
        })?;

        let req = Request::new(params);
        let resp = self.infer(req).await?.into_inner();

        let ptr = wasm.write_message(resp).map_err(|e| {
            Status::aborted(format!("Failed to write infer reply to wasm memory: {e}"))
        })?;
        let param: [Value; 2] = ptr.into();
        println!(
            "Written InferReply. ptr = {:?}-{:?}. param = {param:?}",
            ptr.start, ptr.len
        );

        let cont = call
            .cont
            .ok_or(Status::aborted("Failed to read HostCall: cont is null"))?;

        let reply: DetectionReply = wasm
            .call(&cont.name, &param)
            .map_err(|e| Status::aborted(format!("Failed to call WebAssembly function (2): {e}")))?
            .unwrap_finished();

        println!("Parsed reply: {reply:?}");

        Ok(Response::new(reply))
    }
}
