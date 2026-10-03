use crate::{
    font_engine,
    models::{ActionResult, Operation, RepairProgress},
    transaction,
};
use anyhow::{Context, Result, bail};
use std::{
    fs,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
};
fn status_path() -> Result<PathBuf> {
    Ok(font_engine::data_root()?.join("repair-progress.json"))
}
fn save(progress: &RepairProgress) -> Result<()> {
    let path = status_path()?;
    fs::create_dir_all(font_engine::data_root()?)?;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    transaction::persist_new(&tmp, &serde_json::to_vec(progress)?)?;
    crate::worker::replace_file(&tmp, &path)
}
pub fn progress() -> Result<RepairProgress> {
    let path = status_path()?;
    if !path.exists() {
        return Ok(RepairProgress::default());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
// Windows tools can report a reboot requirement, which must not be treated as ordinary success.
pub fn accepted_exit(code: i32) -> bool {
    matches!(code, 0 | 3010)
}
pub const SYSTEM_FONTS: &[(&str, &str)] = &[
    ("Segoe UI (TrueType)", "segoeui.ttf"),
    ("Segoe UI Bold (TrueType)", "segoeuib.ttf"),
    ("Segoe UI Italic (TrueType)", "segoeuii.ttf"),
    ("Segoe UI Bold Italic (TrueType)", "segoeuiz.ttf"),
    ("Segoe UI Light (TrueType)", "segoeuil.ttf"),
    ("Segoe UI Semibold (TrueType)", "seguisb.ttf"),
    ("Segoe UI Semilight (TrueType)", "segoeuisl.ttf"),
    ("Segoe UI Variable (TrueType)", "SegUIVar.ttf"),
    (
        "Microsoft YaHei & Microsoft YaHei UI (TrueType)",
        "msyh.ttc",
    ),
    (
        "Microsoft YaHei Bold & Microsoft YaHei UI Bold (TrueType)",
        "msyhbd.ttc",
    ),
    (
        "Microsoft YaHei Light & Microsoft YaHei UI Light (TrueType)",
        "msyhl.ttc",
    ),
];
fn restore_system_registration() -> Result<()> {
    use crate::registry::{self, Entry, Scope};
    let windows =
        PathBuf::from(std::env::var_os("WINDIR").context("无法定位 Windows。")?).join("Fonts");
    let mut entries = Vec::new();
    for (name, file) in SYSTEM_FONTS {
        let path = windows.join(file);
        if !path.is_file() {
            continue;
        } // Segoe UI Variable is not present in all Windows versions.
        let bytes = fs::read(&path)?;
        let count = ttf_parser::fonts_in_collection(&bytes).unwrap_or(1);
        let valid = (0..count).any(|index| {
            ttf_parser::Face::parse(&bytes, index).is_ok_and(|f| {
                f.names()
                    .into_iter()
                    .filter(|n| n.name_id == ttf_parser::name_id::FAMILY)
                    .filter_map(|n| n.to_string())
                    .any(|family| {
                        if name.starts_with("Segoe") {
                            family.starts_with("Segoe UI")
                        } else {
                            family.starts_with("Microsoft YaHei")
                        }
                    })
            })
        });
        if !valid {
            bail!("系统字体 {file} 未通过名称校验，未改注册信息。请查看扫描日志。");
        }
        entries.push(Entry {
            slot: registry::slot(Scope::SystemFonts, *name),
            value: Some(registry::string(file)),
        });
        entries.push(Entry {
            slot: registry::slot(Scope::UserSystemFonts, *name),
            value: None,
        });
    }
    if !windows.join("segoeui.ttf").is_file() || !windows.join("msyh.ttc").is_file() {
        bail!("必要的 Windows 字体仍然缺失，未报告修复成功。");
    }
    transaction::execute(
        &mut registry::WindowsRegistry,
        &font_engine::new_backup_dir()?,
        "repair:system-registration",
        entries,
        || Ok(()),
    )?;
    font_engine::notify();
    Ok(())
}
fn run_tool(name: &str, args: &[&str], dir: &std::path::Path) -> Result<i32> {
    let windows = std::env::var_os("WINDIR").context("无法定位 Windows。")?;
    let executable = PathBuf::from(windows).join("System32").join(name);
    let log = fs::File::create(dir.join(format!("{name}.log")))?;
    let status = Command::new(executable)
        .args(args)
        .creation_flags(0x08000000)
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log))
        .status()?;
    let code = status.code().context("系统修复进程未返回退出码。")?;
    fs::write(dir.join(format!("{name}.exit.txt")), code.to_string())?;
    Ok(code)
}
pub fn run() -> Result<ActionResult> {
    // Save managed settings before DISM/SFC starts. Windows repaired files are not a system image backup.
    use crate::registry::{self, Scope, WindowsRegistry};
    let mut slots = std::collections::BTreeSet::new();
    for name in crate::preset_data::MANAGED_ALIASES {
        slots.insert(registry::slot(Scope::Substitutes, *name));
        slots.insert(registry::slot(Scope::Links, *name));
    }
    for name in crate::ui_fonts::NAMES {
        slots.insert(registry::slot(Scope::WindowMetrics, name));
    }
    slots.insert(registry::slot(Scope::LiveUiFonts, "Fonts"));
    for dir in transaction::backup_directories(&font_engine::backup_root()?)? {
        if let Ok(journal) = transaction::read_journal(&dir) {
            slots.extend(journal.before.into_iter().map(|e| e.slot));
        }
    }
    transaction::checkpoint(
        &WindowsRegistry,
        &font_engine::new_backup_dir()?,
        "repair:checkpoint",
        slots.into_iter().collect(),
    )?;
    let dir = font_engine::data_root()?.join("RepairLogs").join(format!(
        "{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir)?;
    let mut p = RepairProgress {
        running: true,
        stage: "正在扫描 Windows 组件…".into(),
        log_dir: dir.to_string_lossy().into(),
    };
    save(&p)?;
    let result = (|| {
        let dism = run_tool(
            "dism.exe",
            &["/Online", "/Cleanup-Image", "/RestoreHealth", "/NoRestart"],
            &dir,
        );
        p.stage = "正在扫描系统文件…".into();
        save(&p)?;
        // Still run SFC even if DISM fails, retaining both outcomes and logs.
        let sfc = run_tool("sfc.exe", &["/scannow"], &dir);
        p.stage = "正在恢复字体映射…".into();
        save(&p)?;
        // Explicit repair can recover defaults even when an old snapshot is unreadable.
        font_engine::perform(Operation::Restore {
            mode: "force-default".into(),
        })?;
        restore_system_registration()?;
        let dism = dism.context("DISM 未能启动。字体映射已恢复，请查看日志。")?;
        let sfc = sfc.context("SFC 未能启动。字体映射已恢复，请查看日志。")?;
        if !accepted_exit(dism) || !accepted_exit(sfc) {
            bail!(
                "字体映射已恢复，但扫描未全部完成（DISM {dism}，SFC {sfc}）。结果：{}",
                dir.display()
            );
        }
        Ok(ActionResult {
            message: format!(
                "扫描已结束，字体映射已恢复。请注销或重启查看效果。修复结果：{}",
                dir.display()
            ),
        })
    })();
    p.running = false;
    p.stage = if result.is_ok() {
        "扫描结束，请注销或重启。".into()
    } else {
        "扫描未完成，请查看修复结果。".into()
    };
    let saved = save(&p);
    result.and_then(|value| {
        saved?;
        Ok(value)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repair_exit_codes_do_not_hide_failure() {
        assert!(accepted_exit(0));
        assert!(accepted_exit(3010));
        assert!(!accepted_exit(5));
        assert!(!accepted_exit(-1));
    }
}
