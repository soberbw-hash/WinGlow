//! One core registry transaction plus durable user-runtime activation/recovery.
use crate::{
    breeze, font_engine, menu_policy,
    models::{ActionResult, Operation},
    registry::WindowsRegistry,
    shell_engine, taskbar, transaction, worker,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    token: String,
    breeze_before: bool,
    taskbar_before: bool,
    #[serde(default)]
    start_menu_applied: bool,
    #[serde(default)]
    window_material_applied: bool,
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
            if bytes.len() > 2048 {
                bail!("优化恢复记录过大。");
            }
            let s: Session = serde_json::from_slice(&bytes)?;
            uuid::Uuid::parse_str(&s.token)?;
            Ok(Some(s))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
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
    font_engine::apply_with_extra("harmonyos-sc-bold", entries, &format!("optimize:{token}"))
}
pub fn undo_core(token: &str) -> Result<ActionResult> {
    let Some(dir) = core_dir(token)? else {
        return Ok(ActionResult {
            message: "未写入系统设置。".into(),
        });
    };
    if dir.join("undone").exists() || dir.join("rolled-back").exists() {
        return Ok(ActionResult {
            message: "系统设置已还原。".into(),
        });
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
    Ok(ActionResult {
        message: "系统设置已还原。".into(),
    })
}
pub fn restore() -> Result<ActionResult> {
    let _guard = worker::optimization_lock()?;
    restore_inner()
}
fn restore_inner() -> Result<ActionResult> {
    let Some(s) = session()? else {
        return Ok(ActionResult {
            message: "没有需要撤销的一键优化。".into(),
        });
    };
    // Stop only effects activated by this session before restoring their configuration.
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
        true,
        crate::explorer::restart,
    )
}
pub fn apply() -> Result<ActionResult> {
    let _guard = worker::optimization_lock()?;
    if session()?.is_some() {
        bail!("已有优化记录，请先撤销后再试。");
    }
    shell_engine::ensure_ready()?;
    // Preparation may download Breeze; no host settings change before all preparation succeeds.
    breeze::prepare()?;
    if taskbar::supported() {
        taskbar::prepare()?;
    }
    let start_menu_applied = crate::visual::start_available();
    if start_menu_applied {
        crate::visual::prepare_start()?;
    }
    let s = Session {
        token: uuid::Uuid::new_v4().to_string(),
        breeze_before: breeze::running()?,
        taskbar_before: taskbar::running()?,
        start_menu_applied,
        window_material_applied: crate::window_material::supported(),
    };
    transaction::persist_new(&marker()?, &serde_json::to_vec(&s)?)?;
    finish_session(
        || {
            worker::run_inner(Operation::OptimizeCore {
                token: s.token.clone(),
            })?;
            Ok(())
        },
        || {
            if taskbar::supported() {
                taskbar::reload_owned()?;
            }
            breeze::sync()?;
            if s.start_menu_applied {
                crate::visual::stop_start()?;
                crate::visual::sync_start()?;
            }
            if s.window_material_applied {
                crate::window_material::sync()?;
            }
            Ok(())
        },
        || transaction::persist_new(&done(&s.token)?, b"ok"),
        restore_inner,
    )?;
    let result = crate::explorer::finish(
        Ok(ActionResult {
            message: "已优化字体、菜单与任务栏。".into(),
        }),
        true,
        crate::explorer::restart,
    )?;
    let mut result = result;
    if crate::visual::start_supported() && !s.start_menu_applied {
        result
            .message
            .push_str(" 已有其他 Windhawk 实例，开始菜单美化未重复启用。");
    }
    Ok(result)
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
        message: format!("已隐藏 {count} 个附加菜单。需要时可手动打开。"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[test]
    fn old_sessions_do_not_claim_new_visual_components() {
        let s: Session = serde_json::from_str(r#"{"token":"1b3bc61e-5619-47b4-b81b-0f435268057c","breeze_before":true,"taskbar_before":true}"#).unwrap();
        assert!(!s.start_menu_applied && !s.window_material_applied);
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
