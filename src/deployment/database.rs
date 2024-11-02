use std::{error::Error, path::PathBuf};

use bytes::Bytes;
use uuid::Uuid;

use super::{
    fs::{read_apps, read_binary, write_tgz},
    http::download,
};

#[derive(Debug, Clone)]
pub struct DeploymentDatabase {
    root: PathBuf,
    app_ids: Vec<Uuid>,
}

pub enum Target {
    Onnx,
    Wasm,
}

impl DeploymentDatabase {
    pub fn read_dir(root: PathBuf) -> Result<Self, Box<dyn Error>> {
        let app_ids = read_apps(&app_dir(&root))?;
        Ok(Self { root, app_ids })
    }

    pub async fn insert(&mut self, app_id: Uuid, source: String) -> Result<(), Box<dyn Error>> {
        let bin = download(source).await?;

        let app_path = app_dir(&self.root).join(app_id.to_string());

        write_tgz(&app_path, bin)?;

        self.app_ids.push(app_id);

        Ok(())
    }

    pub async fn get(&mut self, app_id: Uuid, target: Target) -> Result<Bytes, Box<dyn Error>> {
        let dir = app_dir(&self.root).join(app_id.to_string());
        let bytes = read_binary(&dir, target)?;
        Ok(bytes)
    }
}

fn app_dir(root: &PathBuf) -> PathBuf {
    root.join("apps")
}

impl Default for DeploymentDatabase {
    fn default() -> Self {
        let root = PathBuf::from("./.mless");
        Self::read_dir(root).unwrap()
    }
}

impl ToString for Target {
    fn to_string(&self) -> String {
        match self {
            Self::Wasm => "wasm".to_owned(),
            Self::Onnx => "onnx".to_owned(),
        }
    }
}
