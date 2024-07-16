use std::{error::Error, path::Path};

use wonnx::{Session as GpuSession, SessionError};

use tensor::{Inputs, Outputs};

pub struct Session {
    inner: GpuSession,
}

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
