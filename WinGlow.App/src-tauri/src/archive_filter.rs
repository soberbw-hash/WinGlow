use crate::registry::{Entry, Scope, StoredValue};
use anyhow::{Result, bail};
use std::{fs, path::PathBuf};
const SCRIPT: &[u8] = include_bytes!("archive_filter.js");
fn path() -> Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("USERPROFILE")?)
        .join(".breeze-shell/scripts/WinGlow-ArchiveFilter.js"))
}
pub fn read() -> Result<Option<StoredValue>> {
    match fs::read(path()?) {
        Ok(bytes) => Ok(Some(StoredValue { kind: 3, bytes })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn write(value: Option<&StoredValue>) -> Result<()> {
    let path = path()?;
    if let Some(value) = value {
        if value.kind != 3 || value.bytes.len() > 65536 {
            bail!("菜单过滤快照无效。");
        }
        fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        crate::transaction::persist_new(&tmp, &value.bytes)?;
        crate::worker::replace_file(&tmp, &path)?;
    } else {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub fn plan() -> Result<Vec<Entry>> {
    let value = StoredValue {
        kind: 3,
        bytes: SCRIPT.to_vec(),
    };
    if read()?.as_ref() == Some(&value) {
        return Ok(vec![]);
    }
    Ok(vec![Entry {
        slot: crate::registry::slot(Scope::ArchiveFilter, "WinGlow-ArchiveFilter.js"),
        value: Some(value),
    }])
}
