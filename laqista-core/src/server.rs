use std::{error::Error, marker::PhantomData, sync::Arc};

use bytes::Bytes;

use crate::{
    session::Session,
    tensor::{AsInputs, TryFromOutputs},
    wasm::WasmRunner,
};

pub struct AbtsractServer<I: AsInputs, O: TryFromOutputs> {
    session: Arc<Session>,
    module: WasmRunner,
    pub onnx: Bytes,
    pub wasm: Bytes,
    /// Phantom field for combine I/O types for this struct
    phantom: PhantomData<(I, O)>,
}

impl<I: AsInputs, O: TryFromOutputs> AbtsractServer<I, O> {
    pub fn new(session: Session, module: WasmRunner, onnx: Bytes, wasm: Bytes) -> Self {
        Self {
            session: Arc::new(session),
            module,
            onnx,
            wasm,
            phantom: PhantomData,
        }
    }

    pub async fn infer(&self, input: I) -> Result<O, Box<dyn Error>> {
        let input = input.as_inputs();
        let output = self.session.detect(&input).await?;
        let reply = O::try_from(output)?;

        Ok(reply)
    }

    /// `WasmRunner::get_module()` gets its field `.module`.
    /// We need this getter pattern but not `pub module`, because the binary will be directly executed.
    pub fn get_module(&self) -> &WasmRunner {
        &self.module
    }
}
