use std::{error::Error, sync::Arc};

use prost::Message;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use wasmer::{imports, Cranelift, Instance, Module, Store, Value};

use crate::proto::{
    detector_server::Detector, DetectionReply, DetectionRequest, InferReply, InferRequest,
};

// type ServerPointer = Arc<Mutex<AbtsractServer<InferRequest, InferReply>>>;
pub struct FaceServer {
    // inner: ServerPointer
    wasm: Arc<Mutex<WasmInstance>>,
}

struct WasmInstance {
    store: Store,
    module: Module,
}

static WASM: &'static [u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/face_wasm.wasm");

impl FaceServer {
    pub async fn create() -> Result<Self, Box<dyn Error>> {
        let compiler = Cranelift::default();

        let store = Store::new(compiler);
        let module = Module::new(&store, WASM)?;

        let wasm = WasmInstance { store, module };
        let wasm = Arc::new(Mutex::new(wasm));

        Ok(Self { wasm })
    }
}

#[tonic::async_trait]
impl Detector for FaceServer {
    async fn infer(&self, _request: Request<InferRequest>) -> Result<Response<InferReply>, Status> {
        unimplemented!("Model isn't executed right now")
        // let inner_request = request.into_inner();

        // let reply = self
        //     .0
        //     .lock()
        //     .await
        //     .infer(inner_request)
        //     .await
        //     .map_err(|e| Status::aborted(format!("could not run inference: {e}")))?;

        // Ok(Response::new(reply))
    }

    async fn run_detection(
        &self,
        request: Request<DetectionRequest>,
    ) -> Result<Response<DetectionReply>, Status> {
        let mut wasm = self.wasm.lock().await;
        let module = wasm.module.clone();

        let import_object = imports! {};
        let instance = Instance::new(&mut wasm.store, &module, &import_object)
            .map_err(|e| Status::aborted(format!("Failed to create WebAssembly instance: {e}")))?;

        let main = instance.exports.get_function("main").map_err(|e| {
            Status::aborted(format!("Failed to get expported WebAssembly function: {e}"))
        })?;
        let memory = instance.exports.get_memory("memory").map_err(|e| {
            Status::aborted(format!("Failed to get expported WebAssembly memory: {e}"))
        })?;

        let view = memory.view(&mut wasm.store);
        let mut buffer = Vec::new();
        request
            .into_inner()
            .encode(&mut buffer)
            .map_err(|e| Status::aborted(format!("Failed to encode request data: {e}")))?;

        view.write(0, &buffer).map_err(|e| {
            Status::aborted(format!("Failed to write request data to wasm memory: {e}"))
        })?;

        let params = &[Value::I32(1)];
        main.call(&mut wasm.store, params)
            .map_err(|e| Status::aborted(format!("Failed to call WebAssembly function: {e}")))?;

        let reply = DetectionReply {
            label: "EXECUTED!".to_owned(),
            probability: 1.0,
        };

        Ok(Response::new(reply))
    }
}
