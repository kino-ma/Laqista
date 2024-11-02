use std::{
    error::Error,
    fs::ReadDir,
    io::{self, prelude::*, Result as IOResult},
    path::PathBuf,
};

use bytes::Bytes;
use flate2::read::GzDecoder;
use tar::Archive;
use uuid::Uuid;

use super::database::Target;

pub fn read_apps(root: &PathBuf) -> Result<Vec<Uuid>, Box<dyn Error>> {
    let entries = open_dir(root)?;

    let mut app_ids = vec![];

    for e in entries {
        let entry = e?;

        let id_osstr = entry.file_name();
        let id = id_osstr.to_str().ok_or(format!(
            "Invalid UTF-8 sequence in file name: {:?}",
            entry.file_name()
        ))?;

        let parsed = Uuid::try_parse(id)?;
        app_ids.push(parsed);
    }

    Ok(app_ids)
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
