use crate::models::{ActionResult, Operation};
use anyhow::{Context, Result, bail};
use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
        Security::{TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_QUERY},
        System::{
            RemoteDesktop::ProcessIdToSessionId,
            SystemInformation::GetWindowsDirectoryW,
            Threading::{
                CREATE_PROCESS_LOGON_FLAGS, CREATE_SUSPENDED, CreateProcessW,
                CreateProcessWithTokenW, GetCurrentProcessId, OpenProcess, OpenProcessToken,
                PROCESS_INFORMATION, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_TERMINATE, QueryFullProcessImageNameW, ResumeThread, STARTUPINFOW,
                TerminateProcess, WaitForSingleObject,
            },
        },
        UI::{
            Shell::IsUserAnAdmin,
            WindowsAndMessaging::{FindWindowW, GetShellWindow, GetWindowThreadProcessId},
        },
    },
    core::{PCWSTR, PWSTR},
};

pub fn needed(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::Apply { .. } | Operation::Restore { .. }
    ) || matches!(operation, Operation::RestoreCategory { category } if category == "all-last" || category == "details")
        || matches!(operation, Operation::Toggle { id, .. } if id == "file-extensions")
}

// Refresh is outside the registry transaction and outside the elevated worker.
// A refresh failure must not report a committed font change as failed or undo its backup.
pub fn finish(
    outcome: Result<ActionResult>,
    refresh: bool,
    restart: impl FnOnce() -> Result<()>,
) -> Result<ActionResult> {
    let mut result = outcome?;
    if refresh {
        match restart() {
            Ok(()) => result.message = "已完成，资源管理器已刷新。部分界面仍需注销后生效。".into(),
            Err(error) => result.message.push_str(&format!(
                " 自动刷新未完成：{error:#}。可在任务管理器中重新启动 Windows 资源管理器。"
            )),
        }
    }
    Ok(result)
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct Prepared {
    process: Handle,
    thread: Handle,
    resumed: bool,
}
impl Prepared {
    fn resume(&mut self) -> Result<()> {
        if unsafe { ResumeThread(self.thread.0) } == u32::MAX {
            return Err(windows::core::Error::from_thread().into());
        }
        self.resumed = true;
        Ok(())
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        if !self.resumed {
            unsafe {
                let _ = TerminateProcess(self.process.0, 1);
            }
        }
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain([0]).collect()
}
fn shell_pid() -> Option<u32> {
    let window = unsafe { GetShellWindow() };
    if window.is_invalid() {
        return None;
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    (pid != 0).then_some(pid)
}
fn explorer_path() -> Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let count = unsafe { GetWindowsDirectoryW(Some(&mut buffer)) } as usize;
    if count == 0 || count >= buffer.len() {
        bail!("无法读取 Windows 目录");
    }
    Ok(PathBuf::from(String::from_utf16_lossy(&buffer[..count])).join("explorer.exe"))
}
fn same_session(pid: u32) -> Result<()> {
    let (mut target, mut current) = (0, 0);
    unsafe {
        ProcessIdToSessionId(pid, &mut target)?;
        ProcessIdToSessionId(GetCurrentProcessId(), &mut current)?;
    }
    if target != current {
        bail!("资源管理器不属于当前会话");
    }
    Ok(())
}
fn validate_target(handle: HANDLE, expected: &std::path::Path) -> Result<()> {
    let mut buffer = vec![0u16; 32768];
    let mut size = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )?;
    }
    let actual = String::from_utf16_lossy(&buffer[..size as usize]);
    if !actual.eq_ignore_ascii_case(&expected.to_string_lossy()) {
        bail!("当前桌面不是 Windows 资源管理器，跳过刷新");
    }
    Ok(())
}
fn prepare(executable: &std::path::Path, shell: HANDLE) -> Result<Prepared> {
    let path = wide(executable.as_os_str());
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    // Prepare a suspended replacement BEFORE stopping the shell. If creation fails,
    // the user's current desktop stays alive. Never launch an elevated Explorer.
    unsafe {
        if IsUserAnAdmin().as_bool() {
            let mut token = HANDLE::default();
            OpenProcessToken(
                shell,
                TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY,
                &mut token,
            )?;
            let token = Handle(token);
            CreateProcessWithTokenW(
                token.0,
                CREATE_PROCESS_LOGON_FLAGS(0),
                PCWSTR(path.as_ptr()),
                None,
                CREATE_SUSPENDED,
                None,
                PCWSTR::null(),
                &startup,
                &mut info,
            )?;
        } else {
            CreateProcessW(
                PCWSTR(path.as_ptr()),
                None,
                None,
                None,
                false,
                CREATE_SUSPENDED,
                None,
                PCWSTR::null(),
                &startup,
                &mut info,
            )?;
        }
    }
    Ok(Prepared {
        process: Handle(info.hProcess),
        thread: Handle(info.hThread),
        resumed: false,
    })
}

fn restart_shell() -> Result<()> {
    let old_pid = shell_pid().context("未找到当前桌面的资源管理器")?;
    same_session(old_pid)?;
    let executable = explorer_path()?;
    let shell = Handle(unsafe {
        OpenProcess(
            PROCESS_TERMINATE
                | PROCESS_QUERY_LIMITED_INFORMATION
                | windows::Win32::System::Threading::PROCESS_SYNCHRONIZE,
            false,
            old_pid,
        )
    }?);
    validate_target(shell.0, &executable)?;
    let mut replacement = prepare(&executable, shell.0).context("无法准备资源管理器")?;
    // Recheck ownership before terminating; do not kill processes by name or process tree.
    if shell_pid() != Some(old_pid) {
        bail!("桌面已发生变化，请重试");
    }
    unsafe { TerminateProcess(shell.0, 0) }.context("无法停止资源管理器")?;
    let stopped = unsafe { WaitForSingleObject(shell.0, 5000) };
    // Give Windows' automatic shell recovery time before resuming our fallback.
    // Windows may already have restarted the shell. In that case discard the
    // suspended replacement rather than opening an extra folder window.
    let automatic_deadline = Instant::now() + Duration::from_secs(2);
    while shell_pid().is_none_or(|pid| pid == old_pid) && Instant::now() < automatic_deadline {
        thread::sleep(Duration::from_millis(100));
    }
    if shell_pid().is_none_or(|pid| pid == old_pid) {
        replacement.resume().context("无法重新启动资源管理器")?;
    }
    if stopped != WAIT_OBJECT_0 {
        bail!("等待原资源管理器退出超时");
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Some(pid) = shell_pid().filter(|pid| *pid != old_pid) {
            same_session(pid)?;
            let current =
                Handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }?);
            validate_target(current.0, &executable)?;
            return Ok(());
        }
        thread::sleep(Duration::from_millis(150));
    }
    bail!("未能确认桌面已恢复")
}

static OBSERVED_SHELL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[derive(Default)]
struct StableTaskbar {
    candidate: Option<(u32, isize)>,
    since: Option<Instant>,
}
impl StableTaskbar {
    fn ready(&mut self, candidate: Option<(u32, isize)>, now: Instant) -> bool {
        if candidate != self.candidate {
            self.candidate = candidate;
            self.since = candidate.map(|_| now);
        }
        candidate.is_some()
            && self
                .since
                .is_some_and(|since| now.duration_since(since) >= Duration::from_secs(2))
    }
}
pub(crate) fn wait_for_taskbar() -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut stable = StableTaskbar::default();
    while Instant::now() < deadline {
        let candidate = shell_pid().and_then(|pid| {
            let window = unsafe { FindWindowW(windows::core::w!("Shell_TrayWnd"), None) }.ok()?;
            let mut owner = 0;
            unsafe {
                GetWindowThreadProcessId(window, Some(&mut owner));
            }
            (owner == pid).then_some((pid, window.0 as isize))
        });
        if stable.ready(candidate, Instant::now()) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("等待桌面和任务栏稳定超时，美化组件暂未启动")
}

fn protected_refresh(
    pause: impl FnOnce() -> Result<()>,
    refresh: impl FnOnce() -> Result<()>,
    recover: impl FnOnce() -> Result<()>,
) -> Result<()> {
    // If the owned runtime cannot exit, leave Explorer alive. Even a failed
    // pause/refresh attempts to restore the previously enabled effects.
    let result = pause().and_then(|()| refresh());
    finish_runtime(result, recover)
}

fn finish_runtime(refresh: Result<()>, recover: impl FnOnce() -> Result<()>) -> Result<()> {
    let recovery = recover();
    match (refresh, recovery) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error.context("桌面已刷新，美化组件恢复未完成")),
        (Err(error), Err(recovery)) => bail!("{error:#}；美化组件恢复未完成：{recovery:#}"),
    }
}
fn recover_effects() -> Result<()> {
    let taskbar = (|| {
        if crate::taskbar::enabled()? {
            // Rebind the owned hooks to the replacement Explorer, retaining exact config.
            crate::taskbar::sync()?;
        }
        Ok(())
    })();
    let breeze = (|| {
        if crate::breeze::enabled()? {
            crate::breeze::sync()?;
        }
        Ok(())
    })();
    let visual = crate::visual::sync();
    finish_runtime(finish_runtime(taskbar, || breeze), || visual)
}
pub fn restart() -> Result<()> {
    let result = protected_refresh(crate::taskbar::pause_owned, restart_shell, || {
        wait_for_taskbar()?;
        recover_effects()
    });
    if result.is_ok() {
        OBSERVED_SHELL.store(
            shell_pid().unwrap_or(0),
            std::sync::atomic::Ordering::Relaxed,
        );
    } else {
        // Leave a recovery pending for the bounded watcher retries.
        OBSERVED_SHELL.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    result
}
pub fn watch_shell() {
    OBSERVED_SHELL.store(
        shell_pid().unwrap_or(0),
        std::sync::atomic::Ordering::Relaxed,
    );
    thread::spawn(|| {
        let mut retry_pid = 0;
        let mut attempts = 0;
        loop {
            thread::sleep(Duration::from_secs(2));
            let Some(pid) = shell_pid() else {
                continue;
            };
            if OBSERVED_SHELL.load(std::sync::atomic::Ordering::Relaxed) == pid {
                continue;
            }
            let Ok(_guard) = crate::worker::optimization_lock() else {
                continue;
            };
            if OBSERVED_SHELL.load(std::sync::atomic::Ordering::Relaxed) == pid {
                continue;
            }
            if crate::shell_engine::ensure_ready().is_err()
                || !matches!(crate::optimization::state(), Ok((_, false)))
            {
                continue;
            }
            if retry_pid != pid {
                retry_pid = pid;
                attempts = 0;
            }
            // External Explorer restarts also need a fresh runtime: stop before
            // waiting so another rapid shell restart cannot reach its old hooks.
            let recovery = crate::taskbar::pause_owned().and_then(|()| {
                wait_for_taskbar()?;
                recover_effects()
            });
            match recovery {
                Ok(()) => {
                    OBSERVED_SHELL.store(
                        shell_pid().unwrap_or(0),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
                Err(error) => {
                    attempts += 1;
                    if attempts >= 3 {
                        OBSERVED_SHELL.store(pid, std::sync::atomic::Ordering::Relaxed);
                        eprintln!("美化组件恢复未完成：{error:#}");
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_stops_runtime_before_shell_and_always_recovers() {
        let events = std::cell::RefCell::new(Vec::new());
        protected_refresh(
            || {
                events.borrow_mut().push("pause");
                Ok(())
            },
            || {
                events.borrow_mut().push("shell");
                Ok(())
            },
            || {
                events.borrow_mut().push("recover");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*events.borrow(), ["pause", "shell", "recover"]);
        events.borrow_mut().clear();
        assert!(
            protected_refresh(
                || {
                    events.borrow_mut().push("pause");
                    bail!("stop failed")
                },
                || panic!("Explorer must stay alive when pause fails"),
                || {
                    events.borrow_mut().push("recover");
                    Ok(())
                },
            )
            .is_err()
        );
        assert_eq!(*events.borrow(), ["pause", "recover"]);
        events.borrow_mut().clear();
        assert!(
            protected_refresh(
                || {
                    events.borrow_mut().push("pause");
                    Ok(())
                },
                || {
                    events.borrow_mut().push("shell");
                    bail!("shell failed")
                },
                || {
                    events.borrow_mut().push("recover");
                    Ok(())
                },
            )
            .is_err()
        );
        assert_eq!(*events.borrow(), ["pause", "shell", "recover"]);
    }

    #[test]
    fn stability_requires_same_shell_and_taskbar_without_gaps() {
        let start = Instant::now();
        let mut stable = StableTaskbar::default();
        assert!(!stable.ready(Some((1, 10)), start));
        assert!(!stable.ready(Some((1, 10)), start + Duration::from_secs(1)));
        assert!(stable.ready(Some((1, 10)), start + Duration::from_secs(2)));
        assert!(!stable.ready(None, start + Duration::from_secs(3)));
        assert!(!stable.ready(Some((2, 20)), start + Duration::from_secs(4)));
        assert!(!stable.ready(Some((2, 21)), start + Duration::from_secs(6)));
        assert!(stable.ready(Some((2, 21)), start + Duration::from_secs(8)));
    }

    #[test]
    #[ignore = "Explicit real-desktop acceptance only: restarts Explorer twice"]
    fn real_desktop_two_refreshes_keep_owned_effects_and_config() {
        let _guard = crate::worker::optimization_lock().unwrap();
        crate::shell_engine::ensure_ready().unwrap();
        assert!(
            crate::taskbar::enabled().unwrap(),
            "Taskbar effect must already be enabled"
        );
        crate::taskbar::preflight().unwrap();
        let config = crate::taskbar::read_config().unwrap();
        let breeze_enabled = crate::breeze::enabled().unwrap();
        let start = Instant::now();
        for _ in 0..2 {
            let before = shell_pid().unwrap();
            restart().unwrap();
            assert_ne!(shell_pid(), Some(before));
            assert!(crate::taskbar::running().unwrap());
            assert!(!crate::taskbar::warning_dialog_visible().unwrap());
            assert_eq!(crate::taskbar::read_config().unwrap(), config);
            if breeze_enabled {
                assert!(crate::breeze::running().unwrap());
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "Test must exercise the upstream 30s threshold"
        );
        thread::sleep(Duration::from_secs(3));
        assert!(crate::taskbar::running().unwrap());
        assert!(!crate::taskbar::warning_dialog_visible().unwrap());
        assert_eq!(crate::taskbar::read_config().unwrap(), config);
    }

    #[test]
    fn extension_switch_and_details_restore_refresh_but_label_switch_does_not() {
        assert!(needed(&Operation::Toggle {
            id: "file-extensions".into(),
            enabled: true
        }));
        assert!(needed(&Operation::RestoreCategory {
            category: "details".into()
        }));
        assert!(!needed(&Operation::Toggle {
            id: "desktop-labels".into(),
            enabled: true
        }));
    }
    #[test]
    fn runtime_recovery_runs_even_after_a_shell_error_and_reports_both_failures() {
        let mut recovered = false;
        let error = finish_runtime(Err(anyhow::anyhow!("shell failure")), || {
            recovered = true;
            bail!("runtime failure")
        })
        .unwrap_err()
        .to_string();
        assert!(recovered && error.contains("shell failure") && error.contains("runtime failure"));
        assert!(finish_runtime(Ok(()), || Ok(())).is_ok());
        assert!(finish_runtime(Ok(()), || bail!("runtime failure")).is_err());
    }
    #[test]
    fn failed_mutation_never_restarts_shell() {
        let result = finish(Err(anyhow::anyhow!("write failed")), true, || {
            panic!("must not restart")
        });
        assert!(result.is_err());
    }
    #[test]
    fn refresh_failure_preserves_success_and_reports_recovery() {
        let result = finish(
            Ok(ActionResult {
                message: "已应用。".into(),
            }),
            true,
            || bail!("launch failed"),
        )
        .unwrap();
        assert!(result.message.starts_with("已应用。"));
        assert!(result.message.contains("launch failed"));
        assert!(result.message.contains("任务管理器"));
    }
    #[test]
    fn import_and_repair_do_not_restart_shell() {
        assert!(!needed(&Operation::Import { paths: vec![] }));
        assert!(!needed(&Operation::Repair));
        let result = finish(
            Ok(ActionResult {
                message: "导入完成".into(),
            }),
            false,
            || panic!("must not restart"),
        )
        .unwrap();
        assert_eq!(result.message, "导入完成");
    }
}
