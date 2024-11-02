use std::{error::Error, path::PathBuf};

use uuid::Uuid;

use super::fs::read_apps;

pub struct DeploymentDatabase {
    root: PathBuf,
    app_ids: Vec<Uuid>,
}

impl DeploymentDatabase {
    pub fn read_dir(root: PathBuf) -> Result<Self, Box<dyn Error>> {
        let app_ids = read_apps(&root)?;
        Ok(Self { root, app_ids })
    }
}

impl Default for DeploymentDatabase {
    fn default() -> Self {
        let root = PathBuf::from("./.mless");
        Self::read_dir(root).unwrap()
    }
}
