use crate::{
    font_engine, menu,
    models::{ActionResult, ShellState, ToggleState},
    registry::{self, Entry, Scope, Slot, WindowsRegistry},
    transaction::{self, ValueStore},
};
use anyhow::{Result, bail};
use std::{collections::BTreeMap, fs};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};

pub fn notify() {
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
}
pub fn dword(value: u32) -> registry::StoredValue {
    registry::StoredValue {
        kind: 4,
        bytes: value.to_le_bytes().to_vec(),
    }
}
pub fn number(value: Option<registry::StoredValue>, default: u32) -> u32 {
    value
        .filter(|v| v.kind == 4 && v.bytes.len() == 4)
        .map(|v| u32::from_le_bytes(v.bytes.try_into().unwrap()))
        .unwrap_or(default)
}
pub fn ensure_ready() -> Result<()> {
    if transaction::backup_directories(&font_engine::backup_root()?)?
        .iter()
        .any(|d| transaction::is_pending(d))
    {
        bail!("有未完成的修改，请先撤销最近修改。");
    }
    Ok(())
}
pub fn baseline(slots: &[Slot]) -> Result<Vec<Entry>> {
    let mut original = BTreeMap::new();
    for dir in transaction::backup_directories(&font_engine::backup_root()?)?
        .iter()
        .rev()
    {
        if !(dir.join("committed").exists() || transaction::is_pending(dir)) {
            continue;
        }
        for entry in transaction::read_journal(dir)?.before {
            if slots.contains(&entry.slot) {
                original.entry(entry.slot.clone()).or_insert(entry);
            }
        }
    }
    slots
        .iter()
        .map(|slot| {
            Ok(original.get(slot).cloned().unwrap_or(Entry {
                slot: slot.clone(),
                value: WindowsRegistry.read(slot)?,
            }))
        })
        .collect()
}
pub fn commit(
    operation: &str,
    entries: Vec<Entry>,
    activate: impl FnOnce() -> Result<()>,
) -> Result<ActionResult> {
    ensure_ready()?;
    let desktop_before = if entries.iter().any(|e| e.slot.scope == Scope::DesktopView) {
        Some(crate::desktop::labels_hidden()?)
    } else {
        None
    };
    transaction::execute_with_view(
        &mut WindowsRegistry,
        &font_engine::new_backup_dir()?,
        operation,
        entries,
        desktop_before,
        activate,
    )?;
    notify();
    Ok(ActionResult {
        message: "已保存。部分设置需要注销后生效。".into(),
    })
}
fn tweak_slot(id: &str) -> Result<Slot> {
    Ok(match id {
        "shortcut-arrow" => registry::slot(Scope::IconOverrides, "29"),
        "shield-overlay" => registry::slot(Scope::IconOverrides, "77"),
        "desktop-labels" => registry::slot(Scope::DesktopView, "FFlags"),
        "desktop-icons" => registry::slot(Scope::Explorer, "HideIcons"),
        "file-extensions" => registry::slot(Scope::Explorer, "HideFileExt"),
        _ => bail!("未知美化设置。"),
    })
}
pub fn load() -> Result<ShellState> {
    let mut tweaks = Vec::new();
    for (id, label, note) in [
        ("shortcut-arrow", "隐藏快捷方式箭头", None),
        (
            "shield-overlay",
            "隐藏盾牌角标",
            Some("实验功能，部分 Windows 版本可能不生效。"),
        ),
    ] {
        let value = WindowsRegistry.read(&tweak_slot(id)?)?;
        let enabled = match id {
            "shortcut-arrow" | "shield-overlay" => value
                .and_then(|v| registry::as_string(&v))
                .is_some_and(|s| s == blank_icon_path().unwrap_or_default()),
            "desktop-labels" => {
                crate::desktop::labels_hidden().unwrap_or(number(value, 0) & 0x20000 != 0)
            }
            "file-extensions" => number(value, 1) == 0,
            _ => number(value, 0) != 0,
        };
        tweaks.push(ToggleState {
            id: id.into(),
            label: label.into(),
            enabled,
            note: note.map(str::to_string),
        });
    }
    let (optimization_active, optimization_pending) = crate::optimization::state()?;
    Ok(ShellState {
        items: menu::scan()?.into_iter().map(|(i, _)| i).collect(),
        tweaks,
        breeze_enabled: crate::breeze::enabled()?,
        taskbar_enabled: crate::taskbar::enabled()?,
        taskbar_supported: crate::taskbar::supported(),
        start_menu_enabled: crate::visual::start_enabled()?,
        start_menu_supported: crate::visual::start_supported(),
        window_material_enabled: crate::window_material::enabled()?,
        window_material_supported: crate::window_material::supported(),
        appearance: crate::visual::appearance()?,
        optional_tools: crate::optional_tools::states()?,
        optimization_active,
        optimization_pending,
        pending_recovery: transaction::backup_directories(&font_engine::backup_root()?)?
            .iter()
            .any(|d| transaction::is_pending(d)),
    })
}
fn blank_icon_path() -> Result<String> {
    Ok(format!(
        "{},0",
        font_engine::data_root()?
            .join("transparent.ico")
            .to_string_lossy()
    ))
}
fn prepare_icon() -> Result<()> {
    // Original transparent 1x1 BGRA icon, with a complete AND mask; no system resource edits.
    let path = font_engine::data_root()?.join("transparent.ico");
    let mut ico = vec![
        0, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 0, 32, 0, 48, 0, 0, 0, 22, 0, 0, 0,
    ];
    ico.extend(40u32.to_le_bytes());
    ico.extend(1i32.to_le_bytes());
    ico.extend(2i32.to_le_bytes());
    ico.extend(1u16.to_le_bytes());
    ico.extend(32u16.to_le_bytes());
    ico.extend([0u8; 24]);
    ico.extend([0u8; 4]);
    ico.extend([255, 0, 0, 0]);
    fs::create_dir_all(font_engine::data_root()?)?;
    if !path.exists() {
        transaction::persist_new(&path, &ico)?;
    }
    if fs::read(&path)? != ico {
        bail!("透明图标资源校验失败。");
    }
    Ok(())
}
pub fn overlay_plan() -> Result<Vec<Entry>> {
    prepare_icon()?;
    let path = blank_icon_path()?;
    Ok(["29", "77"]
        .into_iter()
        .map(|name| Entry {
            slot: registry::slot(Scope::IconOverrides, name),
            value: Some(registry::string(&path)),
        })
        .collect())
}
pub fn toggle(id: &str, enabled: bool) -> Result<ActionResult> {
    if matches!(id, "desktop-icons" | "file-extensions" | "desktop-labels") {
        bail!("此美化选项已移除，旧修改仍可通过还原基础美化撤销。");
    }
    if id == "transparent-taskbar" {
        return crate::taskbar::set(enabled);
    }
    if id == "breeze" {
        return crate::breeze::set(enabled);
    }
    if id == "start-menu" {
        return crate::visual::set_start(enabled);
    }
    if id == "window-material" {
        return crate::window_material::set(enabled);
    }
    let slot = tweak_slot(id)?;
    let original = baseline(std::slice::from_ref(&slot))?.remove(0);
    let old = WindowsRegistry.read(&slot)?;
    let value = match id {
        "shortcut-arrow" | "shield-overlay" => {
            if enabled {
                prepare_icon()?;
                Some(registry::string(&blank_icon_path()?))
            } else {
                original.value.filter(|v| {
                    registry::as_string(v)
                        .is_none_or(|s| s != blank_icon_path().unwrap_or_default())
                })
            }
        }
        "desktop-labels" => {
            let flags = number(old, crate::desktop::flags()?);
            Some(dword(if enabled {
                flags | 0x20000
            } else {
                flags & !0x20000
            }))
        }
        "file-extensions" => {
            if enabled {
                Some(dword(0))
            } else {
                original
                    .value
                    .filter(|v| number(Some(v.clone()), 1) != 0)
                    .or(Some(dword(1)))
            }
        }
        _ => {
            if enabled {
                Some(dword(1))
            } else {
                original.value.filter(|v| number(Some(v.clone()), 0) == 0)
            }
        }
    };
    let before_live = if id == "desktop-labels" {
        Some(crate::desktop::labels_hidden()?)
    } else {
        None
    };
    let result = commit(&format!("tweak:{id}"), vec![Entry { slot, value }], || {
        if id == "desktop-labels" {
            crate::desktop::set_labels_hidden(enabled)?;
        }
        Ok(())
    });
    if result.is_err()
        && let Some(old) = before_live
    {
        let _ = crate::desktop::set_labels_hidden(old);
    }
    result
}
pub fn toggle_menu(id: &str, enabled: bool) -> Result<ActionResult> {
    let mut entries = menu::resolve(id, enabled)?;
    entries.extend(crate::archive_filter::plan_with(&entries)?);
    commit("menu:toggle", entries, || Ok(()))
}
pub fn restore(category: &str) -> Result<ActionResult> {
    if !matches!(category, "menu" | "details") {
        bail!("未知恢复类别。");
    }
    let mut slots = Vec::new();
    for dir in transaction::backup_directories(&font_engine::backup_root()?)? {
        if !(dir.join("committed").exists() || transaction::is_pending(&dir)) {
            continue;
        }
        let j = transaction::read_journal(&dir)?;
        for entry in j.before {
            let belongs = match category {
                "menu" => matches!(
                    entry.slot.scope,
                    Scope::MenuVerb { .. }
                        | Scope::MenuHandler { .. }
                        | Scope::CommandStoreVerb { .. }
                        | Scope::BlockedExtensions
                        | Scope::MachineBlockedExtensions
                        | Scope::Startup
                        | Scope::ArchiveFilter
                ),
                _ => matches!(
                    entry.slot.scope,
                    Scope::Explorer
                        | Scope::IconOverrides
                        | Scope::DesktopView
                        | Scope::TaskbarConfig
                        | Scope::VisualConfig
                ),
            };
            let belongs = belongs
                && !(category == "menu"
                    && entry.slot.scope == Scope::Startup
                    && matches!(
                        entry.slot.name.as_str(),
                        "WinGlow-TranslucentTB" | "WinGlow-StartMenu" | "WinGlow-WindowMaterial"
                    ));
            let belongs = belongs
                || category == "details"
                    && entry.slot.scope == Scope::Startup
                    && matches!(
                        entry.slot.name.as_str(),
                        "WinGlow-TranslucentTB" | "WinGlow-StartMenu" | "WinGlow-WindowMaterial"
                    );
            if belongs && !slots.contains(&entry.slot) {
                slots.push(entry.slot);
            }
        }
    }
    if slots.is_empty() {
        return Ok(ActionResult {
            message: "没有需要还原的修改。".into(),
        });
    }
    let mut entries = baseline(&slots)?;
    let mut labels = entries
        .iter()
        .find(|e| e.slot.scope == Scope::DesktopView)
        .map(|e| number(e.value.clone(), 0) & 0x20000 != 0);
    if labels.is_some() {
        for dir in transaction::backup_directories(&font_engine::backup_root()?)?
            .iter()
            .rev()
        {
            if !(dir.join("committed").exists() || transaction::is_pending(dir)) {
                continue;
            }
            let j = transaction::read_journal(dir)?;
            if j.before.iter().any(|e| e.slot.scope == Scope::DesktopView) {
                labels = j.desktop_labels_before.or(labels);
                break;
            }
        }
    }
    let old_labels = if labels.is_some() {
        Some(crate::desktop::labels_hidden()?)
    } else {
        None
    };
    let stops_breeze = entries.iter().any(|e| {
        e.slot.scope == Scope::Startup
            && matches!(
                e.slot.name.as_str(),
                "WinGlow-Breeze" | "WindowsWeitiao-Breeze"
            )
    }) && crate::breeze::enabled()?;
    preserve_view_bits(&mut entries)?;
    let result = commit(&format!("reset:{category}"), entries, || {
        if stops_breeze {
            crate::breeze::stop()?;
        }
        if let Some(hidden) = labels {
            crate::desktop::set_labels_hidden(hidden)?;
        }
        Ok(())
    });
    if result.is_err()
        && let Some(old) = old_labels
    {
        let _ = crate::desktop::set_labels_hidden(old);
    }
    result
}

pub fn undo() -> Result<ActionResult> {
    let dirs = transaction::backup_directories(&font_engine::backup_root()?)?;
    let source = dirs
        .iter()
        .find(|d| transaction::is_pending(d))
        .or_else(|| {
            dirs.iter()
                .find(|d| d.join("committed").exists() && !d.join("undone").exists())
        });
    let Some(source) = source else {
        return font_engine::perform(crate::models::Operation::Restore {
            mode: "last".into(),
        });
    };
    undo_snapshot(source)?;
    Ok(ActionResult {
        message: "已撤销。字体或菜单的完整恢复可能需要注销。".into(),
    })
}

pub fn recover_pending() -> Result<ActionResult> {
    let sources = transaction::pending_directories(&font_engine::backup_root()?)?;
    // Check every journal before writing; a corrupt backup must not be dismissed.
    for source in &sources {
        transaction::read_journal(source)?;
    }
    for source in &sources {
        undo_snapshot(source)?;
    }
    Ok(ActionResult {
        message: if sources.is_empty() {
            "没有未完成的修改，状态已重新检查。".into()
        } else {
            "已恢复未完成修改前的设置。".into()
        },
    })
}

fn undo_snapshot(source: &std::path::Path) -> Result<()> {
    let j = transaction::read_journal(source)?;
    let labels = j.desktop_labels_before.or_else(|| {
        j.before
            .iter()
            .find(|e| e.slot.scope == Scope::DesktopView)
            .map(|e| number(e.value.clone(), 0) & 0x20000 != 0)
    });
    let before_labels = if labels.is_some() {
        Some(crate::desktop::labels_hidden()?)
    } else {
        None
    };
    let mut entries = j.before;
    let stops_breeze = entries.iter().any(|e| {
        e.slot.scope == Scope::Startup
            && matches!(
                e.slot.name.as_str(),
                "WinGlow-Breeze" | "WindowsWeitiao-Breeze"
            )
    }) && crate::breeze::enabled()?;
    preserve_view_bits(&mut entries)?;
    let result = transaction::execute_with_view(
        &mut WindowsRegistry,
        &font_engine::new_backup_dir()?,
        "undo:last",
        entries,
        before_labels,
        || {
            if stops_breeze {
                crate::breeze::stop()?;
            }
            if let Some(hidden) = labels {
                crate::desktop::set_labels_hidden(hidden)?;
            }
            Ok(())
        },
    );
    if result.is_err()
        && let Some(old) = before_labels
    {
        let _ = crate::desktop::set_labels_hidden(old);
    }
    result?;
    transaction::persist_new(
        &source.join(if transaction::is_pending(source) {
            "recovered"
        } else {
            "undone"
        }),
        b"ok",
    )?;
    notify();
    font_engine::notify();
    Ok(())
}
fn preserve_view_bits(entries: &mut [Entry]) -> Result<()> {
    // Preserve missing values exactly; merge only our bit for existing DWORDs.
    for e in entries.iter_mut() {
        if e.slot.scope == Scope::DesktopView
            && e.value
                .as_ref()
                .is_some_and(|v| v.kind == 4 && v.bytes.len() == 4)
        {
            let current = number(WindowsRegistry.read(&e.slot)?, 0);
            e.value = Some(dword(
                (current & !0x20000) | (number(e.value.clone(), 0) & 0x20000),
            ));
        }
    }
    Ok(())
}
