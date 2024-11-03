use std::{error::Error, path::PathBuf, sync::Arc};

use bytes::Bytes;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::server::{StateCommand, StateSender};

use super::{
    fs::{read_apps, read_binary, write_tgz},
    http::download,
};

#[derive(Debug, Clone)]
pub struct DeploymentDatabase {
    root: PathBuf,
    state_tx: StateSender,
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug)]
struct Inner {
    app_ids: Vec<Uuid>,
    instances: Vec<Uuid>,
}

pub enum Target {
    Onnx,
    Wasm,
}

impl DeploymentDatabase {
    pub fn read_dir(root: PathBuf, tx: StateSender) -> Result<Self, Box<dyn Error>> {
        let inner = Arc::new(Mutex::new(Inner::read(&root)?));
        Ok(Self {
            root,
            state_tx: tx,
            inner,
        })
    }

    pub fn default(tx: StateSender) -> Self {
        let root = PathBuf::from("./.mless");
        Self::read_dir(root, tx).unwrap()
    }

    pub async fn add_instance(
        &mut self,
        app_id: Uuid,
        source: String,
    ) -> Result<(), Box<dyn Error>> {
        let mut inner = self.inner.lock().await;

        if !inner.app_ids.contains(&app_id) {
            self.insert(app_id, source).await?;
        }

        inner.instances.push(app_id);

        self.state_tx.send(StateCommand::Keep).await?;

        Ok(())
    }

    pub async fn insert(&self, app_id: Uuid, source: String) -> Result<(), Box<dyn Error>> {
        let bin = download(source).await?;

        let app_path = app_dir(&self.root).join(app_id.to_string());

        write_tgz(&app_path, bin)?;

        self.inner.lock().await.app_ids.push(app_id);

        Ok(())
    }

    pub async fn get(&mut self, app_id: Uuid, target: Target) -> Result<Bytes, Box<dyn Error>> {
        let dir = app_dir(&self.root).join(app_id.to_string());
        let bytes = read_binary(&dir, target)?;
        Ok(bytes)
    }
}

impl Inner {
    pub fn new() -> Self {
        Self {
            app_ids: vec![],
            instances: vec![],
        }
    }

    pub fn read(root: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let app_ids = read_apps(root)?;

        Ok(Self {
            app_ids,
            instances: vec![],
        })
    }
}

fn app_dir(root: &PathBuf) -> PathBuf {
    root.join("apps")
}

impl ToString for Target {
    fn to_string(&self) -> String {
        match self {
            Self::Wasm => "wasm".to_owned(),
            Self::Onnx => "onnx".to_owned(),
        }
    }
}
