use core::{slice, str};
use std::error::Error;

use prost::Message;
use tonic::{Request, Response, Status};
use wasmer::{imports, Cranelift, FunctionEnv, Instance, Memory, MemoryType, Module, Store, Value};

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
        let compiler = Cranelift::default();

        let mut store = Store::new(compiler);
        let module = Module::new(&store, WASM)
            .map_err(|e| Status::aborted(format!("Failed to create WebAssembly module: {e}")))?;

        struct MyEnv;
        let _env = FunctionEnv::new(&mut store, MyEnv);

        fn _print_str(ptr: u32, len: u32) {
            println!("print_str called!!");
            println!("ptr: {ptr}, len: {len}");
            let slic: &[u8] = unsafe { slice::from_raw_parts(ptr as _, len as _) };
            let text = str::from_utf8(slic).unwrap();
            println!("{text}");
        }
        // let print_typed = Function::new_typed_with_env(&mut store, &env, print_str);

        let memory = Memory::new(&mut store, MemoryType::new(21, None, false))
            .map_err(|e| Status::aborted(format!("Failed to create WebAssembly memory: {e}")))?;
        let import_object = imports! {
            "env" => {
                "memory" => memory.clone(),
            }
        };

        let instance = Instance::new(&mut store, &module, &import_object)
            .map_err(|e| Status::aborted(format!("Failed to create WebAssembly instance: {e}")))?;

        println!("Imports: {:?}", module.imports().collect::<Vec<_>>());
        println!("Exports: {:?}", instance.exports);

        let main = instance.exports.get_function("main").map_err(|e| {
            Status::aborted(format!("Failed to get expported WebAssembly function: {e}"))
        })?;

        let mut buf: Vec<u8> = Vec::new();
        request.into_inner().encode(&mut buf).map_err(|e| {
            Status::aborted(format!("Failed to write request data to wasm memory: {e}"))
        })?;

        let view = memory.view(&mut store);
        view.write(0, &buf).map_err(|e| {
            Status::aborted(format!("Failed to write request data to wasm memory: {e}"))
        })?;
        println!("Written {} bytes", buf.len());

        let params = &[Value::I32(0), Value::I32(buf.len() as _)];

        let out = main
            .call(&mut store, params)
            .map_err(|e| Status::aborted(format!("Failed to call WebAssembly function: {e}")))?;

        let value = out[0].unwrap_i64();
        println!("value: {value:x}");

        let start = value >> 32;
        let len = value & 0xffff_ffff;

        let view = memory.view(&mut store);
        let mut buffer = vec![0; len as usize + 1];
        view.read(start as _, &mut buffer).map_err(|e| {
            Status::aborted(format!("Failed to read WebAssembly memory after call: {e}"))
        })?;

        let call: HostCall = Message::decode(&buffer[..])
            .map_err(|e| Status::aborted(format!("Failed to parse host call: {e}")))?;

        println!("HastCall invoked: {:?}", call);

        let reply = DetectionReply {
            label: "EXECUTED!".to_owned(),
            probability: 42.,
        };

        Ok(Response::new(reply))
    }
}
