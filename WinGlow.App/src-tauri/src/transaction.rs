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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop_labels_before: Option<bool>,
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

pub fn checkpoint(
    store: &impl ValueStore,
    dir: &Path,
    operation: &str,
    slots: Vec<Slot>,
) -> Result<()> {
    let before = slots
        .into_iter()
        .map(|slot| {
            Ok(Entry {
                value: store.read(&slot)?,
                slot,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    validate_entries(&before)?;
    let journal = Journal {
        version: 1,
        operation: operation.into(),
        after: before.clone(),
        before,
        desktop_labels_before: None,
    };
    let bytes = serde_json::to_vec_pretty(&journal)?;
    if bytes.len() > 2_000_000 {
        bail!("备份超过大小限制，未开始修复。");
    }
    fs::create_dir_all(dir)?;
    persist_new(&dir.join("snapshot.json"), &bytes)?;
    persist_new(&dir.join("committed"), b"ok")?;
    Ok(())
}
pub fn execute(
    store: &mut impl ValueStore,
    dir: &Path,
    operation: &str,
    desired: Vec<Entry>,
    activate: impl FnOnce() -> Result<()>,
) -> Result<PathBuf> {
    execute_with_view(store, dir, operation, desired, None, activate)
}
pub fn execute_with_view(
    store: &mut impl ValueStore,
    dir: &Path,
    operation: &str,
    desired: Vec<Entry>,
    desktop_labels_before: Option<bool>,
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
        desktop_labels_before,
    };
    let bytes = serde_json::to_vec_pretty(&journal)?;
    if bytes.len() > 2_000_000 {
        bail!("备份超过大小限制，未修改设置。");
    }
    persist_new(&dir.join("snapshot.json"), &bytes).context("保存备份失败，未修改字体设置。")?;
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
        && !dir.join("quarantined").exists()
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
    #[test]
    fn repair_checkpoint_saves_before_any_mutation_without_writing_settings() {
        let mut store = Fake::default();
        let slot = slot(Scope::Substitutes, "Segoe UI");
        store.values.insert(slot.clone(), string("current"));
        let d = dir();
        checkpoint(&store, &d, "repair:checkpoint", vec![slot.clone()]).unwrap();
        assert_eq!(store.writes, 0);
        assert_eq!(
            read_journal(&d).unwrap().before[0].value,
            Some(string("current"))
        );
        fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn live_desktop_state_is_saved_even_without_persistent_flags() {
        let mut store = Fake::default();
        let d = dir();
        execute_with_view(
            &mut store,
            &d,
            "tweak:desktop-labels",
            vec![Entry {
                slot: slot(Scope::DesktopView, "FFlags"),
                value: Some(crate::shell_engine::dword(0x20000)),
            }],
            Some(true),
            || Ok(()),
        )
        .unwrap();
        let j = read_journal(&d).unwrap();
        assert_eq!(j.desktop_labels_before, Some(true));
        assert_eq!(j.before[0].value, None);
        fs::remove_dir_all(d).unwrap();
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
    fn one_click_settings_restore_config_absence_and_menu_bytes_together() {
        let desired = vec![
            Entry {
                slot: slot(Scope::TaskbarConfig, "settings.json"),
                value: Some(StoredValue {
                    kind: 3,
                    bytes: b"{\"desktop_appearance\":{\"accent\":\"clear\"}}".to_vec(),
                }),
            },
            Entry {
                slot: slot(Scope::Startup, "WinGlow-TranslucentTB"),
                value: Some(string("owned taskbar")),
            },
            Entry {
                slot: slot(Scope::Startup, "WinGlow-Breeze"),
                value: Some(string("owned breeze")),
            },
            Entry {
                slot: slot(
                    Scope::MenuVerb {
                        machine: false,
                        path: "Directory\\Background\\shell\\GitHere".into(),
                    },
                    "LegacyDisable",
                ),
                value: Some(string("")),
            },
        ];
        let mut desired = desired;
        for name in ["29", "77"] {
            desired.push(Entry {
                slot: slot(Scope::IconOverrides, name),
                value: Some(string("owned transparent.ico,0")),
            });
        }
        desired.push(Entry {
            slot: slot(
                Scope::MenuVerb {
                    machine: false,
                    path: "Directory\\shell\\Tools\\shell\\Upload".into(),
                },
                "LegacyDisable",
            ),
            value: Some(string("")),
        });
        desired.push(Entry {
            slot: slot(
                Scope::CommandStoreVerb {
                    name: "vendor.Upload".into(),
                },
                "LegacyDisable",
            ),
            value: Some(string("")),
        });
        for fail in [false, true] {
            let mut store = Fake::default();
            store
                .values
                .insert(desired[3].slot.clone(), string("original menu bytes"));
            let original = store.values.clone();
            let d = dir();
            let result = execute(&mut store, &d, "optimize:test", desired.clone(), || {
                if fail {
                    bail!("activation failed")
                } else {
                    Ok(())
                }
            });
            if fail {
                assert!(result.is_err());
                assert!(d.join("rolled-back").exists());
            } else {
                assert!(result.is_ok());
                let journal = read_journal(&d).unwrap();
                write_and_verify(&mut store, &journal.before).unwrap();
            }
            assert_eq!(store.values, original);
            assert!(store.read(&desired[0].slot).unwrap().is_none());
            fs::remove_dir_all(d).unwrap();
        }
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

    // Exercise real Win32 registry byte/type semantics in an isolated disposable hive.
    #[test]
    fn native_ui_fonts_and_persistent_values_restore_together() {
        // Use a memory store: no SPI_SET calls or system-font writes on the test machine.
        let live_slot = slot(Scope::LiveUiFonts, "Fonts");
        let live_before = crate::ui_fonts::read().unwrap();
        let mut desired = crate::ui_fonts::plan("HarmonyOS Sans SC", true).unwrap();
        // Put the live update before the last failing write to exercise combined rollback.
        let live = desired.pop().unwrap();
        desired.insert(0, live);
        for fail in [false, true] {
            let mut store = Fake::default();
            store.values.insert(live_slot.clone(), live_before.clone());
            store.values.insert(
                slot(Scope::WindowMetrics, "MenuFont"),
                string("original type and bytes"),
            );
            let original = store.values.clone();
            store.fail_at = fail.then_some(3);
            let d = dir();
            let result = execute(
                &mut store,
                &d,
                "apply:harmonyos-sc-bold",
                desired.clone(),
                || Ok(()),
            );
            let journal = read_journal(&d).unwrap();
            assert!(
                journal
                    .before
                    .iter()
                    .any(|e| e.slot.scope == Scope::WindowMetrics && e.value.is_none())
            );
            assert_eq!(
                journal
                    .before
                    .iter()
                    .find(|e| e.slot == live_slot)
                    .unwrap()
                    .value,
                Some(live_before.clone())
            );
            if fail {
                assert!(result.is_err());
                assert!(d.join("rolled-back").exists());
            } else {
                result.unwrap();
                write_and_verify(&mut store, &journal.before).unwrap();
                assert!(d.join("committed").exists());
            }
            assert_eq!(store.values, original);
            fs::remove_dir_all(d).unwrap();
        }
    }

    // This test never writes the production font or Explorer keys.
    struct IsolatedRegistry {
        key: winreg::RegKey,
        path: String,
        fail_at: Option<usize>,
        writes: usize,
    }
    impl IsolatedRegistry {
        fn new() -> Self {
            let path = format!(
                r"Software\WinGlow\TransactionTests\{}",
                uuid::Uuid::new_v4()
            );
            let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
                .create_subkey(&path)
                .unwrap()
                .0;
            Self {
                key,
                path,
                fail_at: None,
                writes: 0,
            }
        }
    }
    impl Drop for IsolatedRegistry {
        fn drop(&mut self) {
            assert!(self.path.starts_with(r"Software\WinGlow\TransactionTests\"));
            let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
                .delete_subkey_all(&self.path);
        }
    }
    impl ValueStore for IsolatedRegistry {
        fn read(&self, slot: &Slot) -> Result<Option<StoredValue>> {
            match self.key.get_raw_value(&slot.name) {
                Ok(v) => Ok(Some(StoredValue {
                    kind: v.vtype as u32,
                    bytes: v.bytes.to_vec(),
                })),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.into()),
            }
        }
        fn write(&mut self, e: &Entry) -> Result<()> {
            self.writes += 1;
            if self.fail_at == Some(self.writes) {
                bail!("injected native write failure");
            }
            if let Some(v) = &e.value {
                let kind = match v.kind {
                    1 => winreg::enums::REG_SZ,
                    4 => winreg::enums::REG_DWORD,
                    7 => winreg::enums::REG_MULTI_SZ,
                    _ => bail!("unsupported test value"),
                };
                self.key.set_raw_value(
                    &e.slot.name,
                    &winreg::RegValue {
                        bytes: v.bytes.clone().into(),
                        vtype: kind,
                    },
                )?;
            } else {
                match self.key.delete_value(&e.slot.name) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            Ok(())
        }
    }
    #[test]
    fn native_registry_exact_type_and_absence_are_restored() {
        let mut store = IsolatedRegistry::new();
        let a = slot(Scope::IconOverrides, "29");
        let b = slot(Scope::DesktopView, "FFlags");
        let c = slot(Scope::Substitutes, "Segoe UI");
        let previous = vec![
            Entry {
                slot: a.clone(),
                value: Some(string("用户的原图标,0")),
            },
            Entry {
                slot: b.clone(),
                value: Some(crate::shell_engine::dword(0x12345678)),
            },
            Entry {
                slot: c.clone(),
                value: None,
            },
        ];
        write_and_verify(&mut store, &previous).unwrap();
        let d = dir();
        execute(
            &mut store,
            &d,
            "tweak:integration",
            vec![
                Entry {
                    slot: a.clone(),
                    value: None,
                },
                Entry {
                    slot: b.clone(),
                    value: Some(crate::shell_engine::dword(0x20000)),
                },
                Entry {
                    slot: c.clone(),
                    value: Some(string("changed")),
                },
            ],
            || Ok(()),
        )
        .unwrap();
        let j = read_journal(&d).unwrap();
        assert_eq!(j.before, previous);
        write_and_verify(&mut store, &j.before).unwrap();
        assert_eq!(store.read(&c).unwrap(), None);
        assert_eq!(store.read(&b).unwrap(), previous[1].value);
        fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn native_registry_partial_failure_rolls_back_before_reporting_error() {
        let mut store = IsolatedRegistry::new();
        let original = Entry {
            slot: slot(Scope::Substitutes, "Segoe UI"),
            value: Some(string("原始字形")),
        };
        store.write(&original).unwrap();
        store.writes = 0;
        store.fail_at = Some(2);
        let d = dir();
        assert!(execute(&mut store, &d, "integration", desired(), || Ok(())).is_err());
        assert_eq!(store.read(&original.slot).unwrap(), original.value);
        assert_eq!(
            store
                .read(&slot(Scope::Substitutes, "Microsoft YaHei"))
                .unwrap(),
            None
        );
        assert!(d.join("rolled-back").is_file());
        fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn corrupt_snapshot_is_not_used_and_quarantine_preserves_it() {
        let d = dir();
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("snapshot.json"), b"{broken").unwrap();
        assert!(is_pending(&d));
        assert!(read_journal(&d).is_err());
        persist_new(&d.join("quarantined"), b"explicit repair").unwrap();
        assert!(!is_pending(&d));
        assert_eq!(fs::read(d.join("snapshot.json")).unwrap(), b"{broken");
        fs::remove_dir_all(d).unwrap();
    }
}
