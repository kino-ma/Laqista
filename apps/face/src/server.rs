use std::error::Error;

use mless_core::{proto::host::HostCall, wasm::WasmRunner};
use tonic::{Request, Response, Status};
use wasmer::Value;

use crate::proto::{
    detector_server::Detector, DetectionReply, DetectionRequest, InferReply, InferRequest,
};

// type ServerPointer = Arc<Mutex<AbtsractServer<InferRequest, InferReply>>>;
pub struct FaceServer {
    // inner: ServerPointer
}

static WASM: &'static [u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/face_wasm.wasm");

impl FaceServer {
    pub async fn create() -> Result<Self, Box<dyn Error>> {
        // let compiler = Cranelift::default();

        // let store = Store::new(compiler);
        // let module = Module::new(&store, WASM)?;

        // let wasm = WasmInstance { store, module };
        // let wasm = Arc::new(Mutex::new(wasm));

        // Ok(Self { wasm })
        Ok(Self {})
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

        let params = &[Value::I32(ptr.start), Value::I32(ptr.len)];

        let call: HostCall = wasm
            .call("main", params)
            .map_err(|e| Status::aborted(format!("Failed to call WebAssembly function: {e}")))?;

        println!("HastCall invoked: {:?}", call);

        let param_ptr = call
            .parameters
            .ok_or(Status::aborted("Failed to read HostCall: cont is null"))?;

        let params: InferRequest = wasm.read_message(param_ptr.into()).map_err(|e| {
            Status::aborted(format!(
                "Failed to read infer parameters from wasm memory: {e}"
            ))
        })?;

        let _req = Request::new(params);
        // let resp = self.infer(req).await?;
        // let resp_buf = resp.into_inner().encode_to_vec();

        let resp = InferReply {
            squeezenet0_flatten0_reshape0: vec![0.5, 0.6, 0.9],
        };

        let ptr = wasm.write_message(resp).map_err(|e| {
            Status::aborted(format!("Failed to write infer reply to wasm memory: {e}"))
        })?;
        let param: [Value; 2] = ptr.into();

        let cont = call
            .cont
            .ok_or(Status::aborted("Failed to read HostCall: cont is null"))?;

        let reply: DetectionReply = wasm.call(&cont.name, &param).map_err(|e| {
            Status::aborted(format!("Failed to call WebAssembly function (2): {e}"))
        })?;

        println!("Parsed reply: {reply:?}");

        Ok(Response::new(reply))
    }
}
