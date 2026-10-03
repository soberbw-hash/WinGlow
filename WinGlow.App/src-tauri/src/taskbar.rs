//! Unmodified, isolated TranslucentTB runtime. Never stop a user's other copy.
use crate::{
    font_engine,
    models::ActionResult,
    registry::{self, Entry, Scope, StoredValue, WindowsRegistry},
    shell_engine,
    transaction::{self, ValueStore},
};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, WPARAM},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
        UI::WindowsAndMessaging::{
            EnumWindows, FindWindowExW, GetWindowThreadProcessId, HWND_MESSAGE, PostMessageW,
            WM_CLOSE,
        },
    },
    core::{BOOL, PCWSTR, PWSTR},
};
const ARCHIVE: &[u8] =
    include_bytes!("../../../third-party/TranslucentTB/TranslucentTB-2026.2-x64.zip");
const HASH: &str = "0dbe8e0255c20e131cde536dcd0ae490d45989a7d360e26d0150dff1922ac420";
const FILES: &[&str] = &[
    "Assets\\SplashScreen.jpeg",
    "Assets\\SplashScreen.scale-400.jpeg",
    "ExplorerHooks.dll",
    "ExplorerTAP.dll",
    "ProgramLog.dll",
    "resources.pri",
    "TranslucentTB.exe",
    "Xaml.dll",
];
fn root() -> Result<PathBuf> {
    Ok(font_engine::data_root()?
        .join("TranslucentTB")
        .join("2026.2"))
}
fn exe() -> Result<PathBuf> {
    Ok(root()?.join("TranslucentTB.exe"))
}
pub fn supported() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
        .ok()
        .and_then(|k| k.get_value::<String, _>("CurrentBuildNumber").ok())
        .and_then(|s| s.parse::<u32>().ok())
        .is_some_and(|b| b >= 22000)
}
pub fn startup_slot() -> registry::Slot {
    registry::slot(Scope::Startup, "WinGlow-TranslucentTB")
}
pub fn enabled() -> Result<bool> {
    Ok(WindowsRegistry
        .read(&startup_slot())?
        .and_then(|v| registry::as_string(&v))
        .is_some_and(|s| s == format!("\"{}\"", exe().unwrap_or_default().display())))
}
pub fn read_config() -> Result<Option<StoredValue>> {
    match fs::read(root()?.join("settings.json")) {
        Ok(bytes) => Ok(Some(StoredValue { kind: 3, bytes })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn write_config(value: Option<&StoredValue>) -> Result<()> {
    validate_config(value)?;
    let path = root()?.join("settings.json");
    if let Some(value) = value {
        if value.kind != 3 || value.bytes.len() > 65536 {
            bail!("任务栏配置快照无效。");
        }
        fs::create_dir_all(root()?)?;
        let tmp = root()?.join(format!("settings-{}.tmp", uuid::Uuid::new_v4()));
        transaction::persist_new(&tmp, &value.bytes)?;
        crate::worker::replace_file(&tmp, &path)?;
    } else {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub fn validate_config(value: Option<&StoredValue>) -> Result<()> {
    if value.is_some_and(|v| v.kind != 3 || v.bytes.len() > 65536) {
        bail!("任务栏配置快照无效。");
    }
    Ok(())
}
fn config() -> Vec<u8> {
    serde_json::to_vec_pretty(&serde_json::json!({"desktop_appearance":{"accent":"clear","color":"#00000000","show_line":false},"visible_window_appearance":{"enabled":false},"maximized_window_appearance":{"enabled":false},"start_opened_appearance":{"enabled":false},"search_opened_appearance":{"enabled":false},"task_view_opened_appearance":{"enabled":false},"battery_saver_appearance":{"enabled":false},"disable_saving":true,"hide_tray":false})).unwrap()
}
pub fn plan() -> Result<Vec<Entry>> {
    Ok(vec![
        Entry {
            slot: registry::slot(Scope::TaskbarConfig, "settings.json"),
            value: Some(StoredValue {
                kind: 3,
                bytes: config(),
            }),
        },
        Entry {
            slot: startup_slot(),
            value: Some(registry::string(&format!("\"{}\"", exe()?.display()))),
        },
    ])
}
pub fn prepare() -> Result<()> {
    if !supported() {
        bail!("透明任务栏目前支持 Windows 11。");
    }
    preflight()?;
    if format!("{:x}", Sha256::digest(ARCHIVE)) != HASH {
        bail!("透明任务栏组件校验失败。");
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(ARCHIVE))?;
    for name in FILES {
        let mut entry = archive.by_name(name)?;
        if entry.size() > 2_000_000 {
            bail!("任务栏组件大小异常。");
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        let path = root()?.join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        if path.exists() {
            if fs::read(&path)? != bytes {
                bail!("任务栏组件被修改，未启动。");
            }
        } else {
            transaction::persist_new(&path, &bytes)?;
        }
    }
    fs::write(
        root()?.join("LICENSE.md"),
        include_str!("../../../licenses/TranslucentTB-GPL-3.0.md"),
    )?;
    fs::write(
        root()?.join("SOURCE.md"),
        include_str!("../../../third-party/TranslucentTB/SOURCE.md"),
    )?;
    Ok(())
}
fn processes() -> Result<Vec<(u32, PathBuf)>> {
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = Vec::new();
    if unsafe { Process32FirstW(handle, &mut entry) }.is_ok() {
        loop {
            let name = String::from_utf16_lossy(
                &entry.szExeFile[..entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(260)],
            );
            if name.eq_ignore_ascii_case("TranslucentTB.exe") {
                let p = unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION,
                        false,
                        entry.th32ProcessID,
                    )
                };
                if let Ok(p) = p {
                    let mut buf = vec![0u16; 32768];
                    let mut len = buf.len() as u32;
                    let result = unsafe {
                        QueryFullProcessImageNameW(
                            p,
                            PROCESS_NAME_WIN32,
                            PWSTR(buf.as_mut_ptr()),
                            &mut len,
                        )
                    };
                    unsafe {
                        let _ = CloseHandle(p);
                    }
                    if result.is_ok() {
                        found.push((
                            entry.th32ProcessID,
                            PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])),
                        ));
                    } else {
                        unsafe {
                            let _ = CloseHandle(handle);
                        }
                        bail!("无法确认正在运行的任务栏工具来源。");
                    }
                } else {
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                    bail!("无法读取已有任务栏工具，请先关闭它。");
                }
            }
            if unsafe { Process32NextW(handle, &mut entry) }.is_err() {
                break;
            }
        }
    }
    unsafe {
        let _ = CloseHandle(handle);
    }
    Ok(found)
}
pub fn preflight() -> Result<()> {
    let own = exe()?;
    if processes()?.iter().any(|(_, path)| {
        !path
            .to_string_lossy()
            .eq_ignore_ascii_case(&own.to_string_lossy())
    }) {
        bail!("已有其他 TranslucentTB 正在运行。请先退出它，再启用透明任务栏。");
    }
    Ok(())
}
unsafe extern "system" fn close_window(window: HWND, param: LPARAM) -> BOOL {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    if pid == param.0 as u32 {
        unsafe {
            let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    BOOL(1)
}
pub fn stop() -> Result<()> {
    let own = exe()?;
    for (pid, path) in processes()? {
        if !path
            .to_string_lossy()
            .eq_ignore_ascii_case(&own.to_string_lossy())
        {
            continue;
        }
        unsafe {
            EnumWindows(Some(close_window), LPARAM(pid as isize))?;
        }
        let mut after = HWND::default();
        loop {
            let next = unsafe {
                FindWindowExW(
                    Some(HWND_MESSAGE),
                    Some(after),
                    PCWSTR::null(),
                    PCWSTR::null(),
                )
            };
            let Ok(next) = next else { break };
            if next.is_invalid() {
                break;
            }
            unsafe {
                let _ = close_window(next, LPARAM(pid as isize));
            }
            after = next;
        }
        let deadline = Instant::now() + Duration::from_secs(4);
        while processes()?.iter().any(|(id, _)| *id == pid) {
            if Instant::now() >= deadline {
                bail!("透明任务栏未退出，请保留备份并重试还原。");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}
pub fn sync() -> Result<()> {
    if !enabled()? {
        return stop();
    }
    if unsafe { windows::Win32::UI::Shell::IsUserAnAdmin() }.as_bool() {
        bail!("请以普通权限打开 WinGlow 后启用透明任务栏。");
    }
    prepare()?;
    if processes()?.iter().any(|(_, p)| {
        p.to_string_lossy()
            .eq_ignore_ascii_case(&exe().unwrap_or_default().to_string_lossy())
    }) {
        return Ok(());
    }
    let mut child = Command::new(exe()?)
        .current_dir(root()?)
        .creation_flags(0x08000000)
        .spawn()
        .context("无法启动透明任务栏。")?;
    std::thread::sleep(Duration::from_millis(800));
    if let Some(status) = child.try_wait()? {
        bail!("透明任务栏启动失败（{status}）。");
    }
    Ok(())
}
pub fn set(enable: bool) -> Result<ActionResult> {
    shell_engine::ensure_ready()?;
    if enable {
        prepare()?;
    }
    let mut entries = if enable {
        plan()?
    } else {
        shell_engine::baseline(&[
            startup_slot(),
            registry::slot(Scope::TaskbarConfig, "settings.json"),
        ])?
    };
    if !enable {
        remove_own_startup(&mut entries, &format!("\"{}\"", exe()?.display()));
    }
    let outcome = shell_engine::commit("details:taskbar", entries, sync);
    if outcome.is_err() {
        let _ = sync();
    }
    outcome
}
fn remove_own_startup(entries: &mut [Entry], expected: &str) {
    for entry in entries.iter_mut().filter(|e| e.slot == startup_slot()) {
        if entry
            .value
            .as_ref()
            .and_then(registry::as_string)
            .is_some_and(|s| s == expected)
        {
            entry.value = None;
        }
    }
}
pub fn running() -> Result<bool> {
    let own = exe()?;
    Ok(processes()?.iter().any(|(_, p)| {
        p.to_string_lossy()
            .eq_ignore_ascii_case(&own.to_string_lossy())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_validation_is_read_only_and_does_not_skip_later_entries() {
        let before = read_config().unwrap();
        let entry = Entry {
            slot: registry::slot(Scope::TaskbarConfig, "settings.json"),
            value: Some(StoredValue {
                kind: 3,
                bytes: b"original exact bytes".to_vec(),
            }),
        };
        registry::validate_entries(std::slice::from_ref(&entry)).unwrap();
        assert_eq!(read_config().unwrap(), before);
        assert!(registry::validate_entries(&[entry.clone(), entry.clone()]).is_err());
        let wrong = Entry {
            slot: registry::slot(Scope::Startup, "unmanaged startup"),
            value: None,
        };
        assert!(registry::validate_entries(&[entry, wrong]).is_err());
        assert_eq!(read_config().unwrap(), before);
    }
    #[test]
    fn turning_off_does_not_reenable_an_owned_startup_baseline() {
        let mut entries = vec![Entry {
            slot: startup_slot(),
            value: Some(registry::string("owned command")),
        }];
        remove_own_startup(&mut entries, "owned command");
        assert_eq!(entries[0].value, None);
        entries[0].value = Some(registry::string("previous unrelated command"));
        remove_own_startup(&mut entries, "owned command");
        assert_eq!(
            registry::as_string(entries[0].value.as_ref().unwrap()).as_deref(),
            Some("previous unrelated command")
        );
    }
    #[test]
    fn pinned_archive_and_configuration_are_complete() {
        assert_eq!(format!("{:x}", Sha256::digest(ARCHIVE)), HASH);
        let mut zip = zip::ZipArchive::new(Cursor::new(ARCHIVE)).unwrap();
        assert_eq!(zip.len(), FILES.len());
        for name in FILES {
            assert!(zip.by_name(name).is_ok());
        }
        let parsed: serde_json::Value = serde_json::from_slice(&config()).unwrap();
        assert_eq!(parsed["desktop_appearance"]["accent"], "clear");
        assert_eq!(parsed["disable_saving"], true);
    }
}
