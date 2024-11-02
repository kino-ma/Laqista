use std::{error::Error, marker::PhantomData};

use crate::{
    session::Session,
    tensor::{AsInputs, TryFromOutputs},
};

pub struct AbtsractServer<I: AsInputs, O: TryFromOutputs> {
    session: Session,
    pub onnx: Vec<u8>,
    pub wasm: Vec<u8>,
    /// Phantom field for combine I/O types for this struct
    phantom: PhantomData<(I, O)>,
}

impl<I: AsInputs, O: TryFromOutputs> AbtsractServer<I, O> {
    pub fn new(session: Session, onnx: Vec<u8>, wasm: Vec<u8>) -> Self {
        Self {
            session,
            onnx,
            wasm,
            phantom: PhantomData,
        }
    }

    pub async fn infer(&mut self, input: I) -> Result<O, Box<dyn Error>> {
        let input = input.as_inputs();
        let output = self.session.detect(&input).await?;
        let reply = O::try_from(output)?;

        Ok(reply)
    }
}
