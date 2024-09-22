use std::error::Error;

use mless_core::wasm::WasmRunner;
use prost::Message;
use tonic::{Request, Response, Status};
use wasmer::Value;

use crate::proto::{
    detector_server::Detector, host_proto::HostCall, DetectionReply, DetectionRequest, InferReply,
    InferRequest,
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
        let wasm = WasmRunner::compile(WASM).map_err(|e| {
            Status::aborted(format!("Failed to compile and setup wasm module: {e}"))
        })?;

        println!("Imports: {:?}", &wasm.module.imports().collect::<Vec<_>>());
        println!("Exports: {:?}", &wasm.instance.exports);

        let main = wasm.instance.exports.get_function("main").map_err(|e| {
            Status::aborted(format!("Failed to get expported WebAssembly function: {e}"))
        })?;

        let mut buf: Vec<u8> = Vec::new();
        request.into_inner().encode(&mut buf).map_err(|e| {
            Status::aborted(format!("Failed to write request data to wasm memory: {e}"))
        })?;

        // `view` value may live longer than notion of `.await` later.
        // We must drop it by surrounding by a block.
        {
            let view = memory.view(&mut store);
            view.write(0, &buf).map_err(|e| {
                Status::aborted(format!("Failed to write request data to wasm memory: {e}"))
            })?;
        }
        println!("Written {} bytes", buf.len());

        let params = &[Value::I32(0), Value::I32(buf.len() as _)];

        let out = main
            .call(&mut store, params)
            .map_err(|e| Status::aborted(format!("Failed to call WebAssembly function: {e}")))?;

        let value = out[0].unwrap_i64();
        println!("value: {value:x}");

        let start = value >> 32;
        let len = value & 0xffff_ffff;

        let mut buffer = vec![0; len as usize];
        {
            let view = memory.view(&mut store);
            view.read(start as _, &mut buffer).map_err(|e| {
                Status::aborted(format!("Failed to read WebAssembly memory after call: {e}"))
            })?;
        }

        println!("Read HostCall buffer: {:?}", &buffer[..]);

        let call: HostCall = Message::decode(&buffer[..])
            .map_err(|e| Status::aborted(format!("Failed to parse host call: {e}")))?;

        println!("HastCall invoked: {:?}", call);

        let param_ptr = call
            .parameters
            .ok_or(Status::aborted("Failed to read HostCall: cont is null"))?;

        let mut buffer = vec![0; param_ptr.len as usize];
        {
            let view = memory.view(&mut store);
            view.read(param_ptr.start as _, &mut buffer).map_err(|e| {
                Status::aborted(format!(
                    "Failed to read invoke params from WebAssembly memory: {e}"
                ))
            })?;
        }

        let params: InferRequest = Message::decode(&buffer[..]).map_err(|e| {
            Status::aborted(format!("Failed to parse infer request parameters: {e}"))
        })?;
        let _req = Request::new(params);
        // let resp = self.infer(req).await?;
        // let resp_buf = resp.into_inner().encode_to_vec();
        let resp_buf = InferReply {
            squeezenet0_flatten0_reshape0: vec![0.5, 0.6, 0.9],
        }
        .encode_to_vec();
        let resp_start = param_ptr.start + param_ptr.len + 1;

        {
            let view = memory.view(&mut store);
            view.write(resp_start, &resp_buf[..]).map_err(|e| {
                Status::aborted(format!(
                    "Failed to write response to WebAssembly memory: {e}"
                ))
            })?;
        }

        let cont = call
            .cont
            .ok_or(Status::aborted("Failed to read HostCall: cont is null"))?;

        let next = instance
            .exports
            .get_function(&cont.name)
            .map_err(|e| Status::aborted(format!("Failed to get continuation: {e}")))?;

        let params = &[Value::I32(resp_start as _), Value::I32(resp_buf.len() as _)];

        let out = next.call(&mut store, params).map_err(|e| {
            Status::aborted(format!("Failed to call WebAssembly function (2): {e}"))
        })?;

        let value = out[0].unwrap_i64();
        println!("value: {value:x}");

        let start = value >> 32;
        let len = value & 0xffff_ffff;

        let mut buffer = vec![0; len as usize];
        {
            let view = memory.view(&mut store);
            view.read(start as _, &mut buffer).map_err(|e| {
                Status::aborted(format!(
                    "Failed to read the reply from WebAssembly memory: {e}"
                ))
            })?;
        }

        println!("Read HostCall buffer: {:?}", &buffer[..]);

        let reply: DetectionReply = Message::decode(&buffer[..])
            .map_err(|e| Status::aborted(format!("Failed to parse reply: {e}")))?;

        println!("Parsed reply: {reply:?}");

        Ok(Response::new(reply))
    }
}
