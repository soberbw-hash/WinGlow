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
            WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId},
        },
    },
    core::{PCWSTR, PWSTR},
};

pub fn needed(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::Apply { .. } | Operation::Restore { .. }
    ) || matches!(operation, Operation::RestoreCategory { category } if category == "all-last")
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

pub fn restart() -> Result<()> {
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
    // Windows may already have restarted the shell. In that case discard the
    // suspended replacement rather than opening an extra folder window.
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

#[cfg(test)]
mod tests {
    use super::*;

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
