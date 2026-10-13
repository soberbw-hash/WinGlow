//! Explicit defaults, distinct from undoing the most recent optimization.
use crate::{
    font_engine,
    models::{ActionResult, Operation},
    registry::{self, Entry, Scope},
    shell_engine, transaction, worker,
};
use anyhow::Result;

fn defaults() -> Vec<Entry> {
    let mut entries = Vec::new();
    for name in [
        "WinGlow-Breeze",
        "WindowsWeitiao-Breeze",
        "WinGlow-TranslucentTB",
        "WinGlow-StartMenu",
        "WinGlow-WindowMaterial",
        "WinGlow-Lively",
    ] {
        entries.push(Entry {
            slot: registry::slot(Scope::Startup, name),
            value: None,
        });
    }
    for name in ["29", "77"] {
        entries.push(Entry {
            slot: registry::slot(Scope::IconOverrides, name),
            value: None,
        });
    }
    for name in crate::visual::CONFIG_NAMES {
        entries.push(Entry {
            slot: registry::slot(Scope::VisualConfig, *name),
            value: None,
        });
    }
    entries.push(Entry {
        slot: registry::slot(Scope::TaskbarConfig, "settings.json"),
        value: None,
    });
    entries.push(Entry {
        slot: registry::slot(Scope::ArchiveFilter, "WinGlow-ArchiveFilter.js"),
        value: None,
    });
    entries.push(Entry {
        slot: registry::slot(Scope::ArchiveFilterRules, "WinGlow-MenuRules.json"),
        value: None,
    });
    entries
}
pub(crate) fn plan() -> Result<Vec<Entry>> {
    let dirs = transaction::backup_directories(&font_engine::backup_root()?)?;
    let mut slots = std::collections::BTreeSet::new();
    for dir in &dirs {
        if !dir.join("committed").exists() {
            continue;
        }
        for entry in transaction::read_journal(dir)?.before {
            if shell_engine::belongs_to_category(&entry.slot, "menu")
                || shell_engine::belongs_to_category(&entry.slot, "details")
            {
                slots.insert(entry.slot);
            }
        }
    }
    let mut entries = shell_engine::baseline(&slots.into_iter().collect::<Vec<_>>())?;
    entries.extend(font_engine::default_restore_plan(false, &dirs)?);
    // Explicit system faces: the earliest snapshot might itself be optimized.
    // Keep readable/custom larger sizes; never change display scaling.
    entries.extend(crate::ui_fonts::windows_default_plan()?);
    entries.extend(defaults());
    Ok(font_engine::unique_entries(entries))
}
pub(crate) fn core() -> Result<ActionResult> {
    shell_engine::ensure_ready()?;
    let entries = plan()?;
    transaction::execute(
        &mut registry::WindowsRegistry,
        &font_engine::new_backup_dir()?,
        "reset:normal-defaults",
        entries,
        || Ok(()),
    )?;
    font_engine::notify();
    shell_engine::notify();
    Ok(ActionResult {
        message: "普通还原设置已保存。".into(),
    })
}
pub fn run() -> Result<ActionResult> {
    let _guard = worker::optimization_lock()?;
    shell_engine::ensure_ready()?;
    registry::validate_entries(&plan()?)?;
    let wallpaper = crate::optional_tools::capture_wallpaper_optimization()?;
    let before = serde_json::to_vec(&wallpaper)?;
    let checkpoint = font_engine::data_root()?.join("normal-restore-wallpaper.json");
    let temp = checkpoint.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    transaction::persist_new(&temp, &before)?;
    worker::replace_file(&temp, &checkpoint)?;
    let result = (|| -> Result<ActionResult> {
        crate::optional_tools::disable_managed_wallpaper()?;
        crate::breeze::pause_owned()?;
        crate::taskbar::pause_owned()?;
        crate::visual::stop_start()?;
        crate::window_material::pause()?;
        finish(
            || worker::run_inner(Operation::RestoreNormal).map(|_| ()),
            crate::optimization::clear_session,
            crate::explorer::restart,
        )?;
        Ok(ActionResult {
            message: "已完成普通还原：默认字体、菜单、图标及桌面效果已恢复；未扫描系统文件。"
                .into(),
        })
    })();
    if result.is_err() {
        // Attempt every runtime independently. Registry failures already roll
        // back the single transaction, retaining the full before snapshot.
        let _ = crate::breeze::sync();
        let _ = crate::taskbar::sync();
        let _ = crate::visual::sync();
        let _ = crate::optional_tools::restore_wallpaper_optimization(&wallpaper);
    } else {
        std::fs::remove_file(checkpoint)?;
    }
    result
}
fn finish(
    core: impl FnOnce() -> Result<()>,
    clear: impl FnOnce() -> Result<()>,
    refresh: impl FnOnce() -> Result<()>,
) -> Result<()> {
    core()?;
    clear()?;
    refresh()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_default_write_restores_the_whole_current_profile() {
        use crate::transaction::ValueStore;
        struct Store {
            values: std::collections::BTreeMap<registry::Slot, registry::StoredValue>,
            writes: usize,
        }
        impl ValueStore for Store {
            fn read(&self, slot: &registry::Slot) -> Result<Option<registry::StoredValue>> {
                Ok(self.values.get(slot).cloned())
            }
            fn write(&mut self, entry: &Entry) -> Result<()> {
                self.writes += 1;
                if self.writes == 4 {
                    anyhow::bail!("injected write failure");
                }
                if let Some(value) = &entry.value {
                    self.values.insert(entry.slot.clone(), value.clone());
                } else {
                    self.values.remove(&entry.slot);
                }
                Ok(())
            }
        }
        let entries = defaults();
        let original = entries
            .iter()
            .map(|e| {
                (
                    e.slot.clone(),
                    registry::StoredValue {
                        kind: 3,
                        bytes: b"previous settings".to_vec(),
                    },
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut store = Store {
            values: original.clone(),
            writes: 0,
        };
        let dir =
            std::env::temp_dir().join(format!("winglow-normal-restore-{}", uuid::Uuid::new_v4()));
        assert!(
            transaction::execute(&mut store, &dir, "test:normal-defaults", entries, || Ok(()))
                .is_err()
        );
        assert_eq!(store.values, original);
        assert!(dir.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn explicit_defaults_never_reenable_an_earlier_optimization() {
        let slot = registry::slot(Scope::Startup, "WinGlow-Breeze");
        let mut before = vec![Entry {
            slot: slot.clone(),
            value: Some(registry::string("older optimization")),
        }];
        before.extend(defaults());
        let plan = font_engine::unique_entries(before);
        assert!(
            plan.iter()
                .find(|e| e.slot == slot)
                .unwrap()
                .value
                .is_none()
        );
        assert!(plan.iter().all(|e| e.value.is_none()));
        assert!(!plan.iter().any(|e| e.slot.name == "OneDrive"));
    }
    #[test]
    fn failed_core_keeps_session_and_never_refreshes_desktop() {
        assert!(
            finish(
                || anyhow::bail!("write failed"),
                || panic!("must retain recovery session"),
                || panic!("must not restart desktop")
            )
            .is_err()
        );
        let refreshes = std::cell::Cell::new(0);
        finish(
            || Ok(()),
            || Ok(()),
            || {
                refreshes.set(refreshes.get() + 1);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(refreshes.get(), 1);
    }
}
