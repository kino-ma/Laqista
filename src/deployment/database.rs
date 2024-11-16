use std::{error::Error, path::PathBuf, sync::Arc};

use bytes::Bytes;
use chrono::{Local, TimeZone};
use hex::FromHex;
use mless_core::DeploymentInfo;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    server::{StateCommand, StateSender},
    utils::IdMap,
};

use super::{
    fs::{read_apps, read_binary, write_info, write_tgz},
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
    apps: IdMap<SavedApplication>,
    instances: Vec<Uuid>,
}

#[derive(Clone, Debug)]
pub struct SavedApplication {
    pub info: DeploymentInfo,
    pub deployments: Vec<SavedDeployment>,
}

#[derive(Clone, Debug)]
pub struct SavedDeployment {
    timestamp: chrono::DateTime<Local>,
    hash: Hash,
}
type Hash = [u8; 32];

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
        let root = PathBuf::from(".mless");
        Self::read_dir(root, tx).unwrap()
    }

    pub async fn add_instance(
        &mut self,
        deployment: &DeploymentInfo,
    ) -> Result<(), Box<dyn Error>> {
        if self.inner.lock().await.apps.0.get(&deployment.id).is_none() {
            self.add_app(deployment).await?;
        }

        self.inner.lock().await.instances.push(deployment.id);

        self.state_tx.send(StateCommand::Keep).await?;

        Ok(())
    }

    pub async fn add_app(&self, info: &DeploymentInfo) -> Result<(), Box<dyn Error>> {
        let bin = download(info.source.clone()).await?;

        let saved = self.save(info, bin).await?;

        self.inner.lock().await.insert(info, saved);

        Ok(())
    }

    pub async fn get(
        &self,
        info: &DeploymentInfo,
        target: Target,
    ) -> Result<Bytes, Box<dyn Error>> {
        let dir = app_dir(&self.root, info);
        let bytes = read_binary(&dir, target)?;
        Ok(bytes)
    }

    pub async fn lookup(&self, name: &str) -> Option<DeploymentInfo> {
        self.inner
            .lock()
            .await
            .apps
            .0
            .iter()
            .find(|(_, a)| &a.info.name == name)
            .map(|(_, a)| a.info.clone())
    }

    async fn save(
        &self,
        info: &DeploymentInfo,
        tgz: Bytes,
    ) -> Result<SavedDeployment, Box<dyn Error>> {
        let app_path = app_dir(&self.root, info);

        let timestamp = Local::now();
        let hash = sha256(tgz.clone());
        let saved = SavedDeployment { timestamp, hash };

        let dir_name = saved.dir_name();
        let save_path = app_path.join(dir_name);

        write_tgz(&save_path, tgz)?;

        let info_path = app_path.join("info.mless");
        write_info(&info_path, &info)?;

        Ok(saved)
    }
}

impl Inner {
    pub fn read(root: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let dir = app_root_dir(root);
        let apps = read_apps(&dir)?;

        Ok(Self {
            apps,
            instances: vec![],
        })
    }

    pub fn insert(&mut self, info: &DeploymentInfo, saved: SavedDeployment) {
        self.apps
            .0
            .entry(info.id)
            .and_modify(|a| a.deployments.push(saved.clone()))
            .or_insert(SavedApplication::new(info.clone(), vec![saved]));
    }
}

impl SavedApplication {
    pub fn new(info: DeploymentInfo, deployments: Vec<SavedDeployment>) -> Self {
        Self { info, deployments }
    }
}

impl SavedDeployment {
    pub fn read(dir_name: &str) -> Option<Self> {
        let mut ss = dir_name.split("-");

        let ts_str = ss.next()?;
        let ts_int = ts_str.parse().ok()?;
        let timestamp = Local.timestamp_opt(ts_int, 0).single()?;

        let hash_str = ss.next()?;
        let hash = Hash::from_hex(hash_str).ok()?;

        Some(Self { timestamp, hash })
    }

    pub fn dir_name(&self) -> String {
        let ts = self.timestamp.timestamp();
        let hash = hex::encode(self.hash);

        format!("{ts}-{hash}")
    }
}

fn app_root_dir(root: &PathBuf) -> PathBuf {
    root.join("apps")
}

fn app_dir(root: &PathBuf, deployment: &DeploymentInfo) -> PathBuf {
    app_root_dir(root).join(&deployment.name)
}

fn sha256(bin: Bytes) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(bin);
    hasher.finalize().into()
}

impl ToString for Target {
    fn to_string(&self) -> String {
        match self {
            Self::Wasm => "wasm".to_owned(),
            Self::Onnx => "onnx".to_owned(),
        }
    }
}

#[cfg(test)]
mod test {
    use tokio::sync::mpsc;

    use super::*;

    #[tokio::test]
    async fn db_test() {
        let (tx, _) = mpsc::channel(1);
        let db = DeploymentDatabase::read_dir(PathBuf::from("./.mless-test"), tx).unwrap();

        let info = DeploymentInfo {
            id: Uuid::new_v4(),
            name: "test".to_owned(),
            source: "https://github.com/kino-ma/MLess/releases/download/v0.1.0/face_v0.1.0.tgz"
                .to_owned(),
        };
        db.add_app(&info).await.unwrap();
    }
}
