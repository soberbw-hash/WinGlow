use crate::registry::{Entry, Slot, StoredValue, validate_entries};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub trait ValueStore {
    fn read(&self, slot: &Slot) -> Result<Option<StoredValue>>;
    fn write(&mut self, entry: &Entry) -> Result<()>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Journal {
    pub version: u32,
    pub operation: String,
    pub before: Vec<Entry>,
    pub after: Vec<Entry>,
}

pub fn persist_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let outcome = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = outcome {
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

pub fn read_journal(dir: &Path) -> Result<Journal> {
    let path = dir.join("snapshot.json");
    if fs::metadata(&path)?.len() > 2_000_000 {
        bail!("备份文件过大。");
    }
    let journal: Journal =
        serde_json::from_slice(&fs::read(path)?).context("备份文件损坏，请勿继续应用。")?;
    if journal.version != 1 {
        bail!("不支持此备份版本。");
    }
    validate_entries(&journal.before)?;
    validate_entries(&journal.after)?;
    if journal
        .before
        .iter()
        .map(|e| &e.slot)
        .ne(journal.after.iter().map(|e| &e.slot))
    {
        bail!("备份的前后条目不一致。");
    }
    Ok(journal)
}

fn write_and_verify(store: &mut impl ValueStore, entries: &[Entry]) -> Result<()> {
    for entry in entries {
        store.write(entry)?;
    }
    for entry in entries {
        if store.read(&entry.slot)? != entry.value {
            bail!("写入后的字体值与计划不一致：{}。", entry.slot.name);
        }
    }
    Ok(())
}

pub fn execute(
    store: &mut impl ValueStore,
    dir: &Path,
    operation: &str,
    desired: Vec<Entry>,
    activate: impl FnOnce() -> Result<()>,
) -> Result<PathBuf> {
    validate_entries(&desired)?;
    let before = desired
        .iter()
        .map(|e| {
            Ok(Entry {
                slot: e.slot.clone(),
                value: store.read(&e.slot)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    fs::create_dir_all(dir).context("创建备份目录失败。")?;
    let journal = Journal {
        version: 1,
        operation: operation.into(),
        before,
        after: desired,
    };
    persist_new(
        &dir.join("snapshot.json"),
        &serde_json::to_vec_pretty(&journal)?,
    )
    .context("保存备份失败，未修改字体设置。")?;
    let outcome = write_and_verify(store, &journal.after)
        .and_then(|_| activate())
        .and_then(|_| persist_new(&dir.join("committed"), b"ok"));
    if let Err(error) = outcome {
        match write_and_verify(store, &journal.before) {
            Ok(()) => {
                persist_new(&dir.join("rolled-back"), b"ok")
                    .context("已恢复字体值，但无法标记回退，请保留备份。")?;
                bail!("修改失败，已恢复原值：{error:#}");
            }
            Err(rollback_error) => bail!(
                "修改失败且自动回退未完成：{error:#}；{rollback_error:#}。请使用恢复入口。备份：{}",
                dir.display()
            ),
        }
    }
    Ok(dir.to_path_buf())
}

pub fn backup_directories(root: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs = match fs::read_dir(root) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_ok_and(|t| t.is_dir())
                    && e.file_name().to_string_lossy().starts_with("font-v3-")
            })
            .map(|e| e.path())
            .filter(|p| p.join("snapshot.json").is_file())
            .collect::<Vec<_>>(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).context("读取备份列表失败。"),
    };
    dirs.sort();
    dirs.reverse();
    Ok(dirs)
}
pub fn is_pending(dir: &Path) -> bool {
    !dir.join("committed").exists()
        && !dir.join("rolled-back").exists()
        && !dir.join("recovered").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Scope, slot, string};
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Fake {
        values: BTreeMap<Slot, StoredValue>,
        writes: usize,
        fail_at: Option<usize>,
        ignore: bool,
    }
    impl ValueStore for Fake {
        fn read(&self, s: &Slot) -> Result<Option<StoredValue>> {
            Ok(self.values.get(s).cloned())
        }
        fn write(&mut self, e: &Entry) -> Result<()> {
            self.writes += 1;
            if self.fail_at == Some(self.writes) {
                bail!("injected failure");
            }
            if self.ignore {
                return Ok(());
            }
            if let Some(v) = &e.value {
                self.values.insert(e.slot.clone(), v.clone());
            } else {
                self.values.remove(&e.slot);
            }
            Ok(())
        }
    }
    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!("weitiao-test-{}", uuid::Uuid::new_v4()))
    }
    fn desired() -> Vec<Entry> {
        vec![
            Entry {
                slot: slot(Scope::Substitutes, "Segoe UI"),
                value: Some(string("new")),
            },
            Entry {
                slot: slot(Scope::Substitutes, "Microsoft YaHei"),
                value: Some(string("new")),
            },
        ]
    }
    #[test]
    fn partial_write_restores_original_and_absent_values() {
        let mut s = Fake::default();
        s.values
            .insert(slot(Scope::Substitutes, "Segoe UI"), string("original"));
        let old = s.values.clone();
        s.fail_at = Some(2);
        let d = dir();
        assert!(execute(&mut s, &d, "apply", desired(), || Ok(())).is_err());
        assert_eq!(s.values, old);
        assert!(d.join("rolled-back").exists());
        fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn missing_backup_stops_every_write() {
        let d = dir();
        fs::write(&d, b"blocking file").unwrap();
        let mut s = Fake::default();
        assert!(execute(&mut s, &d, "apply", desired(), || Ok(())).is_err());
        assert_eq!(s.writes, 0);
        fs::remove_file(d).unwrap();
    }
    #[test]
    fn activation_failure_rolls_back_every_value() {
        let d = dir();
        let mut s = Fake::default();
        assert!(execute(&mut s, &d, "apply", desired(), || bail!("font rejected")).is_err());
        assert!(s.values.is_empty());
        fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn successful_commit_can_restore_exact_snapshot() {
        let d = dir();
        let mut s = Fake::default();
        execute(&mut s, &d, "apply", desired(), || Ok(())).unwrap();
        let j = read_journal(&d).unwrap();
        assert_eq!(j.before[0].value, None);
        write_and_verify(&mut s, &j.before).unwrap();
        assert!(s.values.is_empty());
        fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn verification_catches_silently_ignored_write() {
        let d = dir();
        let mut s = Fake {
            ignore: true,
            ..Fake::default()
        };
        assert!(execute(&mut s, &d, "apply", desired(), || Ok(())).is_err());
        assert!(!d.join("committed").exists());
        fs::remove_dir_all(d).unwrap();
    }
}
