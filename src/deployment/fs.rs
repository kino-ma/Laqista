use std::{
    error::Error,
    fs::{DirEntry, ReadDir},
    io::{self, prelude::*, Result as IOResult},
    path::PathBuf,
};

use bytes::Bytes;
use flate2::read::GzDecoder;
use mless_core::DeploymentInfo;
use tar::Archive;

use crate::{proto::Deployment, utils::IdMap};

use super::database::{SavedApplication, SavedDeployment, Target};

pub fn read_apps(root: &PathBuf) -> Result<IdMap<SavedApplication>, Box<dyn Error>> {
    let entries = open_dir(root)?;

    let mut map = IdMap::new();

    for e in entries {
        let entry = e?;

        let app = read_per_app(entry)?;
        map.0.insert(app.info.id, app);
    }

    Ok(map)
}

fn read_per_app(app_entry: DirEntry) -> Result<SavedApplication, Box<dyn Error>> {
    let path = app_entry.path();
    let mut v = vec![];
    let mut info = None;

    for e in std::fs::read_dir(path)? {
        let entry = e?;

        if entry.file_type()?.is_file() {
            let deployment = read_info(&entry.path())?;
            info = deployment.try_into().ok();
            continue;
        }

        let dir_name = entry
            .file_name()
            .to_str()
            .ok_or("failod to get file name")?
            .to_owned();

        let deployment =
            SavedDeployment::read(&dir_name).ok_or("failed to parse directory name")?;

        v.push(deployment)
    }

    if let Some(i) = info {
        Ok(SavedApplication::new(i, v))
    } else {
        Err("failed to get id".to_owned())?
    }
}

pub fn write_tgz(path: &PathBuf, tgz: Bytes) -> IOResult<()> {
    // Ensure directory
    open_dir(path)?;

    let tar = GzDecoder::new(&tgz[..]);
    let mut archive = Archive::new(tar);

    let written_files = archive
        .entries()?
        .map(|entry_result| {
            let mut entry = entry_result?;

            let entry_path = entry.path()?;
            let file_name = entry_path
                .file_name()
                .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

            let file_path = path.join(file_name);

            let mut contents = vec![];
            entry.read_to_end(&mut contents)?;

            std::fs::write(&file_path, &contents)?;

            Ok(file_path)
        })
        .collect::<IOResult<Vec<_>>>()?;

    println!("write_tgz: Written files: {:?}", written_files);

    Ok(())
}

pub fn write_info(path: &PathBuf, info: &DeploymentInfo) -> IOResult<()> {
    use prost::Message;

    let proto_info: Deployment = info.clone().into();
    let msg = proto_info.encode_to_vec();

    std::fs::write(path, msg)?;

    Ok(())
}

pub fn read_binary(dir: &PathBuf, target: Target) -> IOResult<Bytes> {
    let mut dir = std::fs::read_dir(dir)?;

    let entry = dir
        .find_map(|e| {
            let entry = e.ok()?;
            if entry.file_name().to_str()?.ends_with(&target.to_string()) {
                Some(entry)
            } else {
                None
            }
        })
        .ok_or(io::Error::new(io::ErrorKind::NotFound, "target not found"))?;

    let buf = std::fs::read(entry.path())?;
    Ok(Bytes::from(buf))
}

fn read_info(path: &PathBuf) -> Result<Deployment, Box<dyn Error>> {
    let contents = std::fs::read(path)?;
    Ok(<Deployment as prost::Message>::decode(&contents[..])?)
}

fn open_dir(path: &PathBuf) -> Result<ReadDir, std::io::Error> {
    use std::io::ErrorKind::*;

    std::fs::read_dir(path).or_else(|e| match e.kind() {
        NotFound => {
            std::fs::create_dir_all(path)?;
            std::fs::read_dir(path)
        }
        _ => Err(e),
    })
}
