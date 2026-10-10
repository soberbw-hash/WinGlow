//! Match the interactive desktop's token rather than assuming every admin is incompatible.
use anyhow::{Context, Result, bail};
use std::os::windows::ffi::OsStrExt;
static ADAPTATION_ERROR: std::sync::OnceLock<String> = std::sync::OnceLock::new();
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{GetTokenInformation, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenIntegrityLevel},
        System::Threading::{
            CREATE_PROCESS_LOGON_FLAGS, CreateProcessWithTokenW, GetCurrentProcess, OpenProcess,
            OpenProcessToken, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, STARTUPINFOW,
        },
        UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId},
    },
    core::{PCWSTR, PWSTR},
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn integrity(process: HANDLE) -> Result<u32> {
    use windows::Win32::Security::{GetSidSubAuthority, GetSidSubAuthorityCount};
    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(process, TOKEN_QUERY, &mut token)?;
    }
    let token = Handle(token);
    let mut needed = 0;
    unsafe {
        let _ = GetTokenInformation(token.0, TokenIntegrityLevel, None, 0, &mut needed);
    }
    if needed == 0 {
        bail!("无法读取桌面权限信息");
    }
    // usize storage guarantees alignment for TOKEN_MANDATORY_LABEL.
    let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        GetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )?;
        let label = &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
        let count = *GetSidSubAuthorityCount(label.Label.Sid);
        if count == 0 {
            bail!("桌面权限标识无效");
        }
        Ok(*GetSidSubAuthority(label.Label.Sid, u32::from(count - 1)))
    }
}

fn shell() -> Result<Handle> {
    let window = unsafe { GetShellWindow() };
    if window.is_invalid() {
        bail!("当前桌面尚未就绪，请稍后重试");
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    if pid == 0 {
        bail!("无法识别当前桌面");
    }
    Ok(Handle(unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)?
    }))
}

fn requires_relaunch(app: u32, desktop: u32) -> bool {
    app > desktop
}

fn user(process: HANDLE) -> Result<Vec<usize>> {
    use windows::Win32::Security::TokenUser;
    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(process, TOKEN_QUERY, &mut token)?;
    }
    let token = Handle(token);
    let mut needed = 0;
    unsafe {
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut needed);
    }
    if needed == 0 {
        bail!("无法读取登录账户信息");
    }
    let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )?;
    }
    Ok(buffer)
}

fn ensure_same_user(desktop: HANDLE) -> Result<()> {
    use windows::Win32::Security::{EqualSid, TOKEN_USER};
    let app = user(unsafe { GetCurrentProcess() })?;
    let shell = user(desktop)?;
    let same = unsafe {
        EqualSid(
            (*app.as_ptr().cast::<TOKEN_USER>()).User.Sid,
            (*shell.as_ptr().cast::<TOKEN_USER>()).User.Sid,
        )
    };
    if same.is_err() {
        bail!(
            "WinGlow 与桌面属于不同账户，为避免修改错误账户，已停止桌面美化。请使用当前登录账户打开软件。"
        );
    }
    Ok(())
}

pub fn ensure_compatible() -> Result<()> {
    let desktop = shell().context("无法检查桌面权限")?;
    ensure_same_user(desktop.0)?;
    let app = integrity(unsafe { GetCurrentProcess() })?;
    let desktop = integrity(desktop.0)?;
    if app != desktop {
        let detail = ADAPTATION_ERROR
            .get()
            .map(String::as_str)
            .unwrap_or("权限验证未通过");
        bail!("自动适配桌面权限未完成：{detail}。本次未执行桌面美化，无需修改系统 UAC 设置。");
    }
    Ok(())
}

pub fn initialize() -> bool {
    match relaunch_if_needed() {
        Ok(relaunched) => relaunched,
        Err(error) => {
            let _ = ADAPTATION_ERROR.set(format!("{error:#}"));
            false
        }
    }
}

// Called only for the GUI, never for elevated workers or material hosts.
pub fn relaunch_if_needed() -> Result<bool> {
    use windows::Win32::Security::{
        DuplicateTokenEx, SecurityImpersonation, TOKEN_ALL_ACCESS, TOKEN_DUPLICATE, TokenPrimary,
    };
    let desktop = shell()?;
    ensure_same_user(desktop.0)?;
    if !requires_relaunch(
        integrity(unsafe { GetCurrentProcess() })?,
        integrity(desktop.0)?,
    ) {
        return Ok(false);
    }
    if std::env::args().any(|arg| arg == "--desktop-adapted") {
        bail!("自动权限适配未完成，已停止重复启动");
    }
    // Resolve the actual running image (including packaged-path aliases).
    let exe = dunce::canonicalize(std::env::current_exe()?)?;
    let path: Vec<u16> = exe.as_os_str().encode_wide().chain([0]).collect();
    let probe = if std::env::args().any(|arg| arg == "--verify-desktop-permissions") {
        " --verify-desktop-permissions"
    } else {
        ""
    };
    let mut command: Vec<u16> = format!("\"{}\" --desktop-adapted{probe}", exe.display())
        .encode_utf16()
        .chain([0])
        .collect();
    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(desktop.0, TOKEN_QUERY | TOKEN_DUPLICATE, &mut token)
            .context("读取桌面启动令牌失败")?;
    }
    let token = Handle(token);
    let mut primary = HANDLE::default();
    unsafe {
        DuplicateTokenEx(
            token.0,
            TOKEN_ALL_ACCESS,
            None,
            SecurityImpersonation,
            TokenPrimary,
            &mut primary,
        )
        .context("复制桌面启动令牌失败")?;
    }
    let primary = Handle(primary);
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessWithTokenW(
            primary.0,
            CREATE_PROCESS_LOGON_FLAGS(0),
            PCWSTR(path.as_ptr()),
            Some(PWSTR(command.as_mut_ptr())),
            Default::default(),
            None,
            PCWSTR::null(),
            &startup,
            &mut info,
        )
        .context("使用桌面权限启动 WinGlow 失败")?;
    }
    let _process = Handle(info.hProcess);
    let _thread = Handle(info.hThread);
    Ok(true)
}

// Read-only acceptance mode: exercises the real GUI adaptation path without starting
// engines or writing Windows settings. Output has a fixed application-data location.
pub fn handle_probe() -> bool {
    if !std::env::args().any(|arg| arg == "--verify-desktop-permissions") {
        return false;
    }
    let result = (|| -> Result<()> {
        let compatible = ensure_compatible();
        let report = serde_json::json!({
            "compatible": compatible.is_ok(),
            "error": compatible.err().map(|error| format!("{error:#}")),
            "appIntegrity": integrity(unsafe { GetCurrentProcess() })?,
            "desktopIntegrity": integrity(shell()?.0)?,
            "adapted": std::env::args().any(|arg| arg == "--desktop-adapted"),
        });
        let root = crate::font_engine::data_root()?;
        std::fs::create_dir_all(&root)?;
        std::fs::write(
            root.join("desktop-permission-verification.json"),
            serde_json::to_vec(&report)?,
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("Desktop permission verification failed: {error:#}");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_permission_matrix() {
        assert!(!requires_relaunch(0x2000, 0x2000));
        assert!(requires_relaunch(0x3000, 0x2000));
        assert!(!requires_relaunch(0x3000, 0x3000)); // UAC disabled / built-in admin desktop
        assert!(!requires_relaunch(0x2000, 0x3000)); // no automatic elevation
    }
    #[test]
    fn reads_current_and_desktop_tokens_without_changes() {
        assert!(integrity(unsafe { GetCurrentProcess() }).unwrap() >= 0x1000);
        assert!(integrity(shell().unwrap().0).unwrap() >= 0x1000);
        ensure_same_user(unsafe { GetCurrentProcess() }).unwrap();
    }
}
