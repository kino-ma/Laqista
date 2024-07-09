use std::{collections::HashMap, error::Error, path::Path};

use wonnx::{
    utils::{InputTensor, OutputTensor},
    Session as GpuSession, SessionError,
};

pub struct Session {
    inner: GpuSession,
}

pub type Inputs<'a> = HashMap<String, InputTensor<'a>>;
pub type Outputs = HashMap<String, OutputTensor>;

impl Session {
    pub fn new(inner: GpuSession) -> Self {
        Self { inner }
    }

    pub async fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, SessionError> {
        let inner = GpuSession::from_path(path).await?;
        Ok(Self::new(inner))
    }

    pub async fn detect<'a>(&mut self, input: &Inputs<'a>) -> Result<Outputs, Box<dyn Error>> {
        let output = self.inner.run(input).await?;
        Ok(output)
    }
}
