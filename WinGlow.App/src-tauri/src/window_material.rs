//! Public Windows 11 backdrop API, without injecting DLLs into DWM.
//! Original values belong to each HWND and survive host restarts as window properties.
use crate::{
    component_runtime as runtime, font_engine,
    registry::{self, Entry, Scope, StoredValue},
    shell_engine, visual,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, WAIT_TIMEOUT,
        },
        Graphics::Dwm::{DWMWA_SYSTEMBACKDROP_TYPE, DwmGetWindowAttribute, DwmSetWindowAttribute},
        System::Threading::{CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject},
        UI::WindowsAndMessaging::{
            EnumWindows, GWL_STYLE, GetClassNameW, GetPropW, GetWindowLongPtrW,
            GetWindowThreadProcessId, IsWindowVisible, RemovePropW, SetPropW, WS_CAPTION,
        },
    },
    core::{BOOL, PCWSTR},
};

const PROP: &str = "WinGlow.OriginalBackdrop.v1";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
}
pub fn supported() -> bool {
    visual::build() >= 22621
}
pub fn enabled() -> Result<bool> {
    Ok(visual::read("material.json")?
        .map(|v| serde_json::from_slice::<Config>(&v.bytes))
        .transpose()?
        .is_some_and(|c| c.enabled))
}
fn startup_slot() -> registry::Slot {
    registry::slot(Scope::Startup, "WinGlow-WindowMaterial")
}
fn command() -> Result<String> {
    Ok(format!(
        "\"{}\" --window-material-host",
        std::env::current_exe()?.display()
    ))
}
pub fn plan() -> Result<Vec<Entry>> {
    Ok(vec![
        Entry {
            slot: visual::config_slot("material.json"),
            value: Some(StoredValue {
                kind: 3,
                bytes: serde_json::to_vec(&Config { enabled: true })?,
            }),
        },
        Entry {
            slot: startup_slot(),
            value: Some(registry::string(&command()?)),
        },
    ])
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
fn material(hwnd: HWND) -> Result<u32> {
    let mut value: u32 = 0;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &mut value as *mut _ as *mut _,
            4,
        )
    }?;
    Ok(value)
}
fn put(hwnd: HWND, value: u32) -> Result<()> {
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &value as *const _ as *const _,
            4,
        )
    }?;
    Ok(())
}
fn eligible(class: &str, style: isize, value: u32) -> bool {
    // Preserve apps' deliberate Mica/acrylic choices and never style taskbar or menus.
    style & WS_CAPTION.0 as isize == WS_CAPTION.0 as isize
        && value <= 1
        && !matches!(
            class,
            "Shell_TrayWnd"
                | "Shell_SecondaryTrayWnd"
                | "Progman"
                | "WorkerW"
                | "Windows.UI.Core.CoreWindow"
                | "ApplicationFrameWindow"
        )
}
unsafe extern "system" fn visit(hwnd: HWND, param: LPARAM) -> BOOL {
    let restore = param.0 != 0;
    let prop = wide(PROP);
    let key = PCWSTR(prop.as_ptr());
    let saved = unsafe { GetPropW(hwnd, key) };
    if !saved.is_invalid() {
        if restore {
            let original = saved.0 as usize as u32 - 1;
            // If the app changed its own material subsequently, leave that choice alone.
            if material(hwnd).is_ok_and(|current| current != 3 || put(hwnd, original).is_ok()) {
                unsafe {
                    let _ = RemovePropW(hwnd, key);
                }
            }
        }
        return BOOL(1);
    }
    if restore || !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return BOOL(1);
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == std::process::id() {
        return BOOL(1);
    }
    let mut name = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut name) };
    let class = String::from_utf16_lossy(&name[..len.max(0) as usize]);
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    if let Ok(original) = material(hwnd)
        && eligible(&class, style, original)
    {
        // Store BEFORE writing. Properties vanish when HWND is destroyed, preventing reuse bugs.
        let save = unsafe { SetPropW(hwnd, key, Some(HANDLE((original as usize + 1) as *mut _))) };
        let applied = if save.is_ok() {
            put(hwnd, 3)
        } else {
            Err(anyhow::anyhow!("保存窗口属性失败：{save:?}"))
        };
        if save.is_ok() && applied.is_err() {
            unsafe {
                let _ = RemovePropW(hwnd, key);
            }
        }
    }
    BOOL(1)
}
fn scan(restore: bool) -> Result<()> {
    unsafe { EnumWindows(Some(visit), LPARAM(isize::from(restore))) }?;
    Ok(())
}
fn heartbeat() -> Result<std::path::PathBuf> {
    Ok(font_engine::data_root()?.join("window-material-host.json"))
}
fn host_running() -> Result<bool> {
    let name = wide("Local\\WinGlowWindowMaterialHost");
    let handle = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) }?;
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    unsafe {
        let _ = CloseHandle(handle);
    }
    Ok(exists)
}
pub fn pause() -> Result<()> {
    if !host_running()? {
        return scan(true);
    }
    let name = wide("Local\\WinGlowWindowMaterialStop");
    let stop = unsafe { CreateEventW(None, false, false, PCWSTR(name.as_ptr())) }?;
    let signaled = unsafe { SetEvent(stop) };
    unsafe {
        let _ = CloseHandle(stop);
    }
    signaled?;
    let until = Instant::now() + Duration::from_secs(4);
    while Instant::now() < until {
        if !host_running()? {
            return scan(true);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("磨砂后台尚未正常退出，未强制关闭；请稍后重试。")
}
pub fn sync() -> Result<()> {
    if enabled()? {
        if !supported() {
            bail!("窗口磨砂需要 Windows 11 22H2 或更新系统。");
        }
        // A shared mutex, not stale process metadata, decides whether to launch.
        let mutex = wide("Local\\WinGlowWindowMaterialHost");
        let handle = unsafe { CreateMutexW(None, false, PCWSTR(mutex.as_ptr())) }?;
        let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        unsafe {
            let _ = CloseHandle(handle);
        }
        if !exists {
            if heartbeat()?.exists() {
                fs::remove_file(heartbeat()?)?;
            }
            runtime::command(&std::env::current_exe()?)
                .arg("--window-material-host")
                .spawn()?;
            let until = Instant::now() + Duration::from_secs(4);
            while Instant::now() < until {
                if heartbeat()?.exists() {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            bail!("窗口磨砂尚未启动。请稍后重试。");
        }
        Ok(())
    } else {
        let until = Instant::now() + Duration::from_secs(4);
        while Instant::now() < until {
            scan(true)?;
            let mutex = wide("Local\\WinGlowWindowMaterialHost");
            let handle = unsafe { CreateMutexW(None, false, PCWSTR(mutex.as_ptr())) }?;
            let running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
            unsafe {
                let _ = CloseHandle(handle);
            }
            if !running {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        bail!("窗口磨砂尚未完成还原，未强制结束窗口进程。")
    }
}
pub fn set(value: bool) -> Result<crate::models::ActionResult> {
    if value && !supported() {
        bail!("窗口磨砂需要 Windows 11 22H2 或更新系统。");
    }
    let slots = vec![visual::config_slot("material.json"), startup_slot()];
    let entries = if value {
        plan()?
    } else {
        shell_engine::baseline(&slots)?
    };
    let result = shell_engine::commit("visual:window-material", entries, sync);
    if result.is_err() {
        let _ = sync();
    }
    result
}
pub fn host() -> Result<()> {
    let mutex = wide("Local\\WinGlowWindowMaterialHost");
    let handle = unsafe { CreateMutexW(None, true, PCWSTR(mutex.as_ptr())) }?;
    struct Guard(HANDLE);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _guard = Guard(handle);
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(());
    }
    if !supported() {
        return Ok(());
    }
    let stop_name = wide("Local\\WinGlowWindowMaterialStop");
    let stop = unsafe { CreateEventW(None, false, false, PCWSTR(stop_name.as_ptr())) }?;
    let _stop_guard = Guard(stop);
    let outcome = (|| {
        fs::write(
            heartbeat()?,
            serde_json::to_vec(
                &serde_json::json!({"pid":std::process::id(),"engine":"Windows DWM API"}),
            )?,
        )?;
        while enabled()? {
            scan(false)?;
            if unsafe { WaitForSingleObject(stop, 500) } != WAIT_TIMEOUT {
                break;
            }
        }
        Ok(())
    })();
    let restored = scan(true);
    let _ = fs::remove_file(heartbeat()?);
    outcome.and(restored)
}

pub fn verify_api() -> Result<serde_json::Value> {
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
    };
    let class = wide("STATIC");
    let title = wide("WinGlow material verification");
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            320,
            160,
            None,
            None,
            None,
            None,
        )
    }?;
    let result = (|| {
        let before = material(hwnd)?;
        put(hwnd, 3)?;
        let applied = material(hwnd)?;
        put(hwnd, before)?;
        let restored = material(hwnd)?;
        if applied != 3 || restored != before {
            bail!("Windows 材质接口写入/还原不一致。");
        }
        Ok(serde_json::json!({"before":before,"applied":applied,"restored":restored}))
    })();
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    result
}
pub fn verify_host() -> Result<serde_json::Value> {
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SW_SHOWNOACTIVATE, ShowWindow, WINDOW_EX_STYLE,
        WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };
    let class = wide("STATIC");
    let title = wide("WinGlow background verification");
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            -10000,
            -10000,
            320,
            160,
            None,
            None,
            None,
            None,
        )
    }?;
    struct Guard(HWND);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    let _guard = Guard(hwnd);
    let before = material(hwnd)?;
    // The verification process is launched with STARTF_USESHOWWINDOW=HIDE;
    // the first ShowWindow obeys that startup override. The second exposes our
    // off-screen fixture without activating it, matching host eligibility.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    let wait = |expected| -> Result<()> {
        let until = Instant::now() + Duration::from_secs(4);
        while Instant::now() < until {
            if material(hwnd)? == expected {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        bail!(
            "磨砂后台状态不一致，预期材质 {expected}，实际 {:?}，创建时 {before}，可见 {}，样式 {:x}。",
            material(hwnd),
            unsafe { IsWindowVisible(hwnd).as_bool() },
            unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) }
        );
    };
    wait(3)?;
    pause()?;
    wait(before)?;
    if !enabled()? {
        bail!("暂停后台不应修改启用设置。");
    }
    sync()?;
    wait(3)?;
    Ok(
        serde_json::json!({"before":before,"applied":3,"pausedRestored":before,"resumed":3,"configurationPreserved":true}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserve_existing_material_and_system_surfaces() {
        let caption = WS_CAPTION.0 as isize;
        assert!(eligible("Notepad", caption, 0));
        assert!(!eligible("Notepad", caption, 2));
        assert!(!eligible("Notepad", caption, 3));
        assert!(!eligible("Shell_TrayWnd", caption, 0));
        assert!(!eligible("Popup", 0, 0));
    }
}
