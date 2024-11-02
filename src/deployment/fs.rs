use std::{error::Error, path::PathBuf};

use uuid::Uuid;

pub fn read_apps(root: &PathBuf) -> Result<Vec<Uuid>, Box<dyn Error>> {
    let entries = std::fs::read_dir(root)?;

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
