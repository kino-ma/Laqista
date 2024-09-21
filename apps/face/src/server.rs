use std::{error::Error, sync::Arc};

use mless_core::{server::AbtsractServer, session::Session};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use wasmer::{
    imports, Cranelift, Function, FunctionEnv, FunctionEnvMut, Instance, Module, Store, Value,
};

use crate::{
    model_path,
    proto::{
        detector_server::Detector, DetectionReply, DetectionRequest, InferReply, InferRequest,
    },
};

type ServerPointer = Arc<Mutex<AbtsractServer<InferRequest, InferReply>>>;
pub struct FaceServer(ServerPointer, Module);

static WASM: &'static [u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/face_wasm.wasm");

impl FaceServer {
    pub async fn create() -> Result<Self, Box<dyn Error>> {
        let compiler = Cranelift::default();
        let mut store = Store::new(compiler);
        let module = Module::new(&store, WASM)?;

        let imports: Vec<_> = module.imports().collect();
        println!("Imports: {imports:?}");
        let exports: Vec<_> = module.exports().collect();
        println!("Imports: {exports:?}");

        struct MyEnv;
        let env = FunctionEnv::new(&mut store, MyEnv);
        fn f(_env: FunctionEnvMut<MyEnv>, fs: i32) -> i32 {
            fs.into()
        }
        let f_typed = Function::new_typed_with_env(&mut store, &env, f);

        let import_object = imports! {
            "env" => {
                "infer" => f_typed,
            },
        };
        let instance = Instance::new(&mut store, &module, &import_object)?;
        let main = instance.exports.get_function("main")?;
        let params = &[Value::I32(1)];
        main.call(&mut store, params)?;

        let path = model_path();
        let session = Session::from_path(path).await?;
        let server = AbtsractServer::new(session);
        let ptr = Arc::new(Mutex::new(server));

        Ok(Self(ptr, module))
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

    async fn run_detection(
        &self,
        _request: Request<DetectionRequest>,
    ) -> Result<Response<DetectionReply>, Status> {
        todo!("wasm を実行する service")
    }
}
