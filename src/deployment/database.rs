use std::{error::Error, path::PathBuf};

use uuid::Uuid;

use super::{
    fs::{read_apps, write_tgz},
    http::download,
};

#[derive(Debug, Clone)]
pub struct DeploymentDatabase {
    root: PathBuf,
    app_ids: Vec<Uuid>,
}

impl DeploymentDatabase {
    pub fn read_dir(root: PathBuf) -> Result<Self, Box<dyn Error>> {
        let app_ids = read_apps(&root)?;
        Ok(Self { root, app_ids })
    }

    pub async fn insert(&mut self, app_id: Uuid, source: String) -> Result<(), Box<dyn Error>> {
        let bin = download(source).await?;

        let app_path = self.app_root().join(app_id.to_string());

        write_tgz(&app_path, bin)?;

        self.app_ids.push(app_id);

        Ok(())
    }

    fn app_root(&self) -> PathBuf {
        self.root.join("apps")
    }
}

impl Default for DeploymentDatabase {
    fn default() -> Self {
        let root = PathBuf::from("./.mless");
        Self::read_dir(root).unwrap()
    }
}
