//! One core registry transaction plus durable user-runtime activation/recovery.
use crate::{
    breeze, font_engine, menu_policy,
    models::{ActionResult, Operation},
    registry::WindowsRegistry,
    shell_engine, taskbar, transaction, worker,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, Mutex};
use std::{fs, path::PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct UpgradeStatus {
    pub running: bool,
    pub finished: bool,
    pub message: String,
    pub error: bool,
}
static UPGRADE: LazyLock<Mutex<UpgradeStatus>> = LazyLock::new(|| {
    Mutex::new(UpgradeStatus {
        running: true,
        finished: false,
        message: String::new(),
        error: false,
    })
});
pub fn upgrade_status() -> UpgradeStatus {
    UPGRADE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
pub fn finish_upgrade(result: &Result<Option<ActionResult>>) {
    let mut status = UPGRADE.lock().unwrap_or_else(|e| e.into_inner());
    status.running = false;
    status.finished = true;
    status.error = false;
    status.message.clear();
    match result {
        Ok(Some(result)) => {
            status.message = format!("已更新优化方案，原始备份仍可撤销。{}", result.message)
        }
        Ok(None) => (),
        Err(error) => {
            status.error = true;
            status.message = format!("桌面初始化未完成：{error:#}");
        }
    }
    // Persist the exact failure for support and avoid repeating a failing automatic
    // migration every time the same version opens. Manual retry stays available.
    if let Ok(root) = font_engine::data_root() {
        let report = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"status":*status});
        let _ = fs::write(
            root.join("optimization-upgrade-status.json"),
            report.to_string(),
        );
    }
}
pub fn update_on_startup() -> Result<Option<ActionResult>> {
    // A historical error must not block initialization after undo/recovery,
    // or after a completed migration whose last shell refresh failed.
    let Some(before) = session()? else {
        return Ok(None);
    };
    if !upgrade_due(&before, done(&before.token)?.is_file()) {
        return Ok(None);
    }
    if let Ok(bytes) = fs::read(font_engine::data_root()?.join("optimization-upgrade-status.json"))
        && let Ok(report) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && report["version"] == env!("CARGO_PKG_VERSION")
        && report["status"]["error"] == true
    {
        bail!(
            "{}",
            report["status"]["message"]
                .as_str()
                .unwrap_or("请查看恢复页面")
                .trim_start_matches("优化方案更新未完成：")
                .trim_start_matches("桌面初始化未完成：")
        );
    }
    update_if_due()
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    token: String,
    breeze_before: bool,
    taskbar_before: bool,
    #[serde(default)]
    start_menu_applied: bool,
    #[serde(default)]
    window_material_applied: bool,
    #[serde(default)]
    wallpaper: Option<crate::optional_tools::WallpaperOptimization>,
    #[serde(default)]
    applied_version: String,
    #[serde(default)]
    prior_tokens: Vec<String>,
}
fn marker() -> Result<PathBuf> {
    Ok(font_engine::data_root()?.join("one-click-session.json"))
}
fn done(token: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(token)?;
    Ok(font_engine::data_root()?.join(format!("one-click-{token}.complete")))
}
fn session() -> Result<Option<Session>> {
    match fs::read(marker()?) {
        Ok(bytes) => {
            if bytes.len() > 16384 {
                bail!("优化恢复记录过大。");
            }
            let s: Session = serde_json::from_slice(&bytes)?;
            uuid::Uuid::parse_str(&s.token)?;
            if s.prior_tokens.len() > 256 {
                bail!("优化恢复记录过多。");
            }
            for token in &s.prior_tokens {
                uuid::Uuid::parse_str(token)?;
            }
            Ok(Some(s))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub(crate) fn clear_session() -> Result<()> {
    if let Some(s) = session()? {
        fs::remove_file(marker()?)?;
        if done(&s.token)?.is_file() {
            fs::remove_file(done(&s.token)?)?;
        }
    }
    Ok(())
}
pub fn state() -> Result<(bool, bool)> {
    if let Some(s) = session()? {
        Ok((true, !done(&s.token)?.exists()))
    } else {
        Ok((false, false))
    }
}
fn core_dir(token: &str) -> Result<Option<PathBuf>> {
    uuid::Uuid::parse_str(token)?;
    for dir in transaction::backup_directories(&font_engine::backup_root()?)? {
        if transaction::read_journal(&dir)?.operation == format!("optimize:{token}") {
            return Ok(Some(dir));
        }
    }
    Ok(None)
}
pub fn core(token: &str) -> Result<ActionResult> {
    uuid::Uuid::parse_str(token)?;
    shell_engine::ensure_ready()?;
    let mut entries = menu_policy::plan()?;
    entries.extend(shell_engine::overlay_plan()?);
    entries.extend(breeze::startup_entries()?);
    if taskbar::supported() {
        entries.extend(taskbar::plan()?);
    }
    let s = session()?.ok_or_else(|| anyhow::anyhow!("缺少优化恢复记录。"))?;
    if s.token != token {
        bail!("优化请求与恢复记录不一致。");
    }
    if s.start_menu_applied {
        entries.extend(crate::visual::start_plan()?);
    }
    if s.window_material_applied {
        entries.extend(crate::window_material::plan()?);
    }
    if s.wallpaper.is_some() {
        entries.push(crate::optional_tools::wallpaper_startup_entry()?);
    }
    font_engine::apply_with_extra("harmonyos-sc-bold", entries, &format!("optimize:{token}"))
}
pub fn undo_core(token: &str) -> Result<ActionResult> {
    let s = session()?.ok_or_else(|| anyhow::anyhow!("缺少优化恢复记录。"))?;
    if s.token != token {
        bail!("优化恢复请求不一致。");
    }
    restore_chain(&s, undo_core_single)?;
    Ok(ActionResult {
        message: "系统设置已还原。".into(),
    })
}
pub fn rollback_upgrade_core(token: &str) -> Result<ActionResult> {
    let s = session()?.ok_or_else(|| anyhow::anyhow!("缺少升级恢复记录。"))?;
    if s.token != token || s.prior_tokens.is_empty() {
        bail!("升级恢复请求不一致。");
    }
    restore_upgrade_only(&s, undo_core_single)?;
    Ok(ActionResult {
        message: "已恢复升级前的系统设置。".into(),
    })
}
fn restore_upgrade_only(s: &Session, mut restore: impl FnMut(&str) -> Result<()>) -> Result<()> {
    restore(&s.token)
}
fn restore_chain(s: &Session, mut restore: impl FnMut(&str) -> Result<()>) -> Result<()> {
    for token in std::iter::once(&s.token).chain(s.prior_tokens.iter().rev()) {
        restore(token)?;
    }
    Ok(())
}
fn undo_core_single(token: &str) -> Result<()> {
    let Some(dir) = core_dir(token)? else {
        return Ok(());
    };
    if dir.join("undone").exists() || dir.join("rolled-back").exists() {
        return Ok(());
    }
    let before = transaction::read_journal(&dir)?.before;
    transaction::execute(
        &mut WindowsRegistry,
        &font_engine::new_backup_dir()?,
        "undo:optimization",
        before,
        || Ok(()),
    )?;
    transaction::persist_new(
        &dir.join(if transaction::is_pending(&dir) {
            "recovered"
        } else {
            "undone"
        }),
        b"ok",
    )?;
    font_engine::notify();
    shell_engine::notify();
    Ok(())
}
pub fn restore() -> Result<ActionResult> {
    let _guard = worker::optimization_lock()?;
    restore_inner(true)
}
pub(crate) fn restore_deferred() -> Result<ActionResult> {
    let _guard = worker::optimization_lock()?;
    restore_inner(false)
}
fn restore_inner(refresh: bool) -> Result<ActionResult> {
    restore_inner_with_wallpaper(refresh, true)
}
fn restore_inner_with_wallpaper(refresh: bool, restore_wallpaper: bool) -> Result<ActionResult> {
    let Some(s) = session()? else {
        return Ok(ActionResult {
            message: "没有需要撤销的一键优化。".into(),
        });
    };
    // UAC cancellation/preparation failure before a core snapshot is a no-op.
    // Do not cycle every runtime and Explorer just to remove a session marker.
    let mut any_core = core_dir(&s.token)?.is_some();
    for token in &s.prior_tokens {
        any_core |= core_dir(token)?.is_some();
    }
    if !any_core {
        fs::remove_file(marker()?)?;
        if done(&s.token)?.exists() {
            fs::remove_file(done(&s.token)?)?;
        }
        return Ok(ActionResult {
            message: "本次操作未修改系统，无需还原。".into(),
        });
    }
    // Stop only effects activated by this session before restoring their configuration.
    if restore_wallpaper && let Some(wallpaper) = &s.wallpaper {
        crate::optional_tools::restore_wallpaper_optimization(wallpaper)?;
    }
    if !s.taskbar_before {
        taskbar::stop()?;
    }
    if !s.breeze_before {
        breeze::stop()?;
    }
    if s.start_menu_applied {
        crate::visual::stop_start()?;
    }
    worker::run_inner(Operation::UndoOptimization {
        token: s.token.clone(),
    })?;
    // Runtime repair happens in the ordinary parent, never inside an elevated worker.
    if s.taskbar_before {
        taskbar::sync()?;
    }
    if s.breeze_before {
        breeze::sync()?;
    }
    if s.start_menu_applied {
        crate::visual::sync_start()?;
    }
    if s.window_material_applied {
        crate::window_material::sync()?;
    }
    fs::remove_file(marker()?)?;
    if done(&s.token)?.exists() {
        fs::remove_file(done(&s.token)?)?;
    }
    crate::explorer::finish(
        Ok(ActionResult {
            message: "已撤销优化，恢复优化前的设置。".into(),
        }),
        refresh,
        crate::explorer::restart,
    )
}
pub fn apply() -> Result<ActionResult> {
    let _guard = worker::optimization_lock()?;
    if session()?.is_some() {
        bail!("已有优化记录，请先撤销后再试。");
    }
    apply_inner(None)
}
pub fn update_if_due() -> Result<Option<ActionResult>> {
    let _guard = worker::optimization_lock()?;
    let Some(before) = session()? else {
        return Ok(None);
    };
    if !upgrade_due(&before, done(&before.token)?.is_file()) {
        return Ok(None);
    }
    apply_inner(Some(before)).map(Some)
}
fn upgrade_due(before: &Session, completed: bool) -> bool {
    migration_due(
        &before.applied_version,
        env!("CARGO_PKG_VERSION"),
        completed,
    )
}
fn migration_due(applied: &str, current: &str, completed: bool) -> bool {
    if !completed {
        return false;
    }
    if applied.is_empty() {
        return true;
    }
    match (
        semver::Version::parse(applied),
        semver::Version::parse(current),
    ) {
        (Ok(applied), Ok(current)) => applied < current,
        _ => false,
    }
}
fn apply_inner(before: Option<Session>) -> Result<ActionResult> {
    shell_engine::ensure_ready()?;
    // Dynamic wallpaper is a large optional download. Keep it out of one-click
    // optimization so core settings can finish without waiting on a 218 MB installer.
    // Preparation may download Breeze; no host settings change before all preparation succeeds.
    breeze::prepare()?;
    if taskbar::supported() {
        taskbar::prepare()?;
    }
    let start_menu_applied = crate::visual::start_available();
    if start_menu_applied {
        crate::visual::prepare_start()?;
    }
    // The original baseline is for explicit undo. A failed upgrade must instead
    // restore the currently working profile, including the user's wallpaper.
    let prior_tokens = before
        .as_ref()
        .map(|old| {
            let mut tokens = old.prior_tokens.clone();
            tokens.push(old.token.clone());
            tokens
        })
        .unwrap_or_default();
    if prior_tokens.len() > 256 {
        bail!("优化恢复记录过多，请先撤销优化。");
    }
    let s = Session {
        token: uuid::Uuid::new_v4().to_string(),
        breeze_before: match &before {
            Some(old) => old.breeze_before,
            None => breeze::running()?,
        },
        taskbar_before: match &before {
            Some(old) => old.taskbar_before,
            None => taskbar::running()?,
        },
        start_menu_applied,
        window_material_applied: crate::window_material::supported(),
        wallpaper: before.as_ref().and_then(|old| old.wallpaper.clone()),
        applied_version: env!("CARGO_PKG_VERSION").into(),
        prior_tokens,
    };
    if before.is_some() {
        let temporary = marker()?.with_extension(format!("{}.tmp", s.token));
        transaction::persist_new(&temporary, &serde_json::to_vec(&s)?)?;
        worker::replace_file(&temporary, &marker()?)?;
    } else {
        transaction::persist_new(&marker()?, &serde_json::to_vec(&s)?)?;
    }
    let mut refresh_message = String::new();
    finish_session(
        || {
            worker::run_inner(Operation::OptimizeCore {
                token: s.token.clone(),
            })?;
            Ok(())
        },
        || {
            // All registry changes are complete. Refresh once before starting
            // new effects/wallpaper, rather than starting and stopping them twice.
            refresh_message = crate::explorer::finish(
                Ok(ActionResult {
                    message: String::new(),
                }),
                true,
                crate::explorer::restart,
            )?
            .message;
            if taskbar::supported() {
                taskbar::sync()?;
            }
            breeze::sync()?;
            if s.start_menu_applied {
                crate::visual::sync_start()?;
            }
            if s.window_material_applied {
                crate::window_material::sync()?;
            }
            Ok(())
        },
        || transaction::persist_new(&done(&s.token)?, b"ok"),
        || {
            if let Some(old) = &before {
                worker::run_inner(Operation::RollbackUpgrade {
                    token: s.token.clone(),
                })?;
                taskbar::sync()?;
                breeze::sync()?;
                crate::visual::reload()?;
                crate::optional_tools::resume_lively_after_refresh()?;
                let temporary = marker()?.with_extension(format!("{}.restore.tmp", s.token));
                transaction::persist_new(&temporary, &serde_json::to_vec(old)?)?;
                worker::replace_file(&temporary, &marker()?)?;
                crate::explorer::finish(
                    Ok(ActionResult {
                        message: "已保留升级前的优化方案。".into(),
                    }),
                    false,
                    crate::explorer::restart,
                )
            } else {
                // Wallpaper has not changed yet. Do not stop it or restart the
                // shell a second time while rolling back a critical failure.
                let restored = restore_inner_with_wallpaper(false, false)?;
                crate::optional_tools::resume_lively_after_refresh()?;
                Ok(restored)
            }
        },
    )?;
    let mut result = ActionResult {
        message: format!("已优化字体、右键菜单、图标角标及桌面效果。{refresh_message}"),
    };
    result.message.push_str(" 动态壁纸可稍后单独下载。");
    if crate::visual::start_supported() && !s.start_menu_applied {
        result
            .message
            .push_str(" 已有其他 Windhawk 实例，开始菜单美化未重复启用。");
    }
    Ok(result)
}
#[cfg(test)]
fn finish_optional_wallpaper(
    apply: impl FnOnce() -> Result<()>,
    restore: impl FnOnce() -> Result<()>,
) -> Result<()> {
    match apply() {
        Ok(()) => Ok(()),
        Err(error) => match restore() {
            Ok(()) => Err(error.context("已恢复原壁纸")),
            Err(restore) => Err(error.context(format!("原壁纸恢复未完成：{restore:#}"))),
        },
    }
}
fn finish_session(
    core: impl FnOnce() -> Result<()>,
    activate: impl FnOnce() -> Result<()>,
    mark: impl FnOnce() -> Result<()>,
    rollback: impl FnOnce() -> Result<ActionResult>,
) -> Result<()> {
    match core().and_then(|_| activate()).and_then(|_| mark()) {
        Ok(()) => Ok(()),
        Err(error) => match rollback() {
            Ok(_) => bail!("优化未完成，已撤销本次修改：{error:#}"),
            Err(restore_error) => {
                bail!("优化未完成：{error:#}。恢复尚未完成：{restore_error:#}。请点击首页恢复。")
            }
        },
    }
}

pub fn menu_only() -> Result<ActionResult> {
    let entries = menu_policy::plan()?;
    let count = entries.len();
    if count == 0 {
        return Ok(ActionResult {
            message: "菜单已简洁，没有可确认的多余项。".into(),
        });
    }
    shell_engine::commit("menu:auto", entries, || Ok(()))?;
    Ok(ActionResult {
        message: "菜单已精简。需要时可手动打开附加选项。".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_wallpaper_failure_restores_only_wallpaper_and_retains_its_error() {
        let restores = std::cell::Cell::new(0);
        let error = finish_optional_wallpaper(
            || bail!("player timeout"),
            || {
                restores.set(restores.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(restores.get(), 1);
        assert!(format!("{error:#}").contains("player timeout"));
        finish_optional_wallpaper(
            || Ok(()),
            || panic!("successful wallpaper must not roll back"),
        )
        .unwrap();
        let error = finish_optional_wallpaper(|| bail!("apply"), || bail!("restore")).unwrap_err();
        assert!(format!("{error:#}").contains("restore"));
    }
    use std::cell::RefCell;
    #[test]
    fn only_completed_older_sessions_need_a_migration() {
        let mut session: Session = serde_json::from_str(
            r#"{"token":"original","breeze_before":false,"taskbar_before":false}"#,
        )
        .unwrap();
        for version in ["", "1.1.9", env!("CARGO_PKG_VERSION")] {
            session.applied_version = version.into();
            assert!(!upgrade_due(&session, false));
            assert_eq!(
                upgrade_due(&session, true),
                version != env!("CARGO_PKG_VERSION")
            );
        }
    }
    #[test]
    fn older_executable_never_downgrades_a_newer_optimization() {
        assert!(!migration_due("1.1.12", "1.1.11", true));
        assert!(!migration_due("broken", "1.1.11", true));
        assert!(!migration_due("1.1.11", "1.1.11", true));
        assert!(migration_due("1.1.9", "1.1.11", true));
    }
    #[test]
    fn old_sessions_do_not_claim_new_visual_components() {
        let s: Session = serde_json::from_str(r#"{"token":"1b3bc61e-5619-47b4-b81b-0f435268057c","breeze_before":true,"taskbar_before":true}"#).unwrap();
        assert!(!s.start_menu_applied && !s.window_material_applied);
        assert!(s.wallpaper.is_none() && s.applied_version.is_empty() && s.prior_tokens.is_empty());
    }
    #[test]
    fn upgrade_restores_latest_first_then_the_original_baseline() {
        let mut s: Session = serde_json::from_str(
            r#"{"token":"latest","breeze_before":false,"taskbar_before":false}"#,
        )
        .unwrap();
        s.prior_tokens = vec!["original".into(), "previous".into()];
        let mut order = vec![];
        restore_chain(&s, |token| {
            order.push(token.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(order, ["latest", "previous", "original"]);
        let mut stopped = vec![];
        assert!(
            restore_chain(&s, |token| {
                stopped.push(token.to_owned());
                if token == "previous" {
                    bail!("failure");
                }
                Ok(())
            })
            .is_err()
        );
        assert_eq!(stopped, ["latest", "previous"]);
        let mut upgrade_rollback = vec![];
        restore_upgrade_only(&s, |token| {
            upgrade_rollback.push(token.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(upgrade_rollback, ["latest"]);
    }
    #[test]
    fn each_failure_stage_rolls_back_and_never_marks_success() {
        for fail in ["core", "activate", "mark", "none"] {
            let events = RefCell::new(Vec::new());
            let stage = |name| {
                events.borrow_mut().push(name);
                if fail == name {
                    bail!("injected {name}");
                }
                Ok(())
            };
            let result = finish_session(
                || stage("core"),
                || stage("activate"),
                || stage("mark"),
                || {
                    events.borrow_mut().push("rollback");
                    Ok(ActionResult {
                        message: "restored".into(),
                    })
                },
            );
            assert_eq!(result.is_ok(), fail == "none");
            let expected = match fail {
                "core" => vec!["core", "rollback"],
                "activate" => vec!["core", "activate", "rollback"],
                "mark" => vec!["core", "activate", "mark", "rollback"],
                _ => vec!["core", "activate", "mark"],
            };
            assert_eq!(*events.borrow(), expected);
        }
    }
    #[test]
    fn failed_recovery_stays_an_error_with_an_actionable_entry() {
        let error = finish_session(
            || Ok(()),
            || bail!("runtime"),
            || Ok(()),
            || bail!("recovery"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("runtime") && error.contains("recovery") && error.contains("首页恢复")
        );
        assert!(!error.contains("已撤销本次修改"));
    }
}

