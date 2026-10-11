//! Change display flags without renaming files or removing shortcut privileges.
use anyhow::{Context, Result};
use windows::{
    Win32::{
        System::{
            Com::{
                CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize, IServiceProvider,
            },
            Variant::VARIANT,
        },
        UI::Shell::{
            FWF_HIDEFILENAMES, IFolderView2, IShellBrowser, IShellWindows, SID_STopLevelBrowser,
            SWC_DESKTOP, SWFO_NEEDDISPATCH, ShellWindows,
        },
    },
    core::Interface,
};
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
fn view() -> Result<(Apartment, IFolderView2)> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .context("无法连接桌面。")?;
        let apartment = Apartment;
        let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL)?;
        let empty = VARIANT::default();
        let mut hwnd = 0;
        let dispatch =
            windows.FindWindowSW(&empty, &empty, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH)?;
        let provider: IServiceProvider = dispatch.cast()?;
        let browser: IShellBrowser = provider.QueryService(&SID_STopLevelBrowser)?;
        let view: IFolderView2 = browser.QueryActiveShellView()?.cast()?;
        Ok((apartment, view))
    }
}
pub fn flags() -> Result<u32> {
    let (_a, v) = view()?;
    Ok(unsafe { v.GetCurrentFolderFlags()? })
}
pub fn labels_hidden() -> Result<bool> {
    Ok(flags()? & FWF_HIDEFILENAMES.0 as u32 != 0)
}
pub fn refresh_font() -> Result<()> {
    use windows::{
        Win32::{
            Foundation::{LPARAM, WPARAM},
            UI::WindowsAndMessaging::{
                FindWindowExW, SMTO_ABORTIFHUNG, SPI_SETICONTITLELOGFONT, SendMessageTimeoutW,
                WM_SETTINGCHANGE,
            },
        },
        core::w,
    };
    // Explorer's desktop view can keep its old font after the profile and SPI
    // font have changed. Notify that view directly; never restart the shell.
    for class in [w!("Progman"), w!("WorkerW")] {
        let mut previous = None;
        while let Ok(parent) = unsafe { FindWindowExW(None, previous, class, None) } {
            previous = Some(parent);
            let Ok(view) =
                (unsafe { FindWindowExW(Some(parent), None, w!("SHELLDLL_DefView"), None) })
            else {
                continue;
            };
            let sent = unsafe {
                SendMessageTimeoutW(
                    view,
                    WM_SETTINGCHANGE,
                    WPARAM(SPI_SETICONTITLELOGFONT.0 as usize),
                    LPARAM(0),
                    SMTO_ABORTIFHUNG,
                    1000,
                    None,
                )
            };
            if sent.0 == 0 {
                anyhow::bail!("桌面文字刷新暂未响应。");
            }
            return Ok(());
        }
    }
    anyhow::bail!("桌面图标视图尚未就绪。");
}
pub fn font_diagnostics() -> Result<serde_json::Value> {
    use windows::{
        Win32::{
            Foundation::{LPARAM, WPARAM},
            Graphics::Gdi::{GetObjectW, HGDIOBJ, LOGFONTW},
            UI::{
                HiDpi::GetDpiForWindow,
                WindowsAndMessaging::{
                    FindWindowExW, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETFONT,
                },
            },
        },
        core::w,
    };
    for class in [w!("Progman"), w!("WorkerW")] {
        let mut previous = None;
        while let Ok(parent) = unsafe { FindWindowExW(None, previous, class, None) } {
            previous = Some(parent);
            let Ok(view) =
                (unsafe { FindWindowExW(Some(parent), None, w!("SHELLDLL_DefView"), None) })
            else {
                continue;
            };
            let Ok(list) = (unsafe { FindWindowExW(Some(view), None, w!("SysListView32"), None) })
            else {
                continue;
            };
            let mut handle = 0usize;
            let sent = unsafe {
                SendMessageTimeoutW(
                    list,
                    WM_GETFONT,
                    WPARAM(0),
                    LPARAM(0),
                    SMTO_ABORTIFHUNG,
                    500,
                    Some(&mut handle),
                )
            };
            if sent.0 == 0 || handle == 0 {
                anyhow::bail!(
                    "桌面未响应字体读取请求（结果 {}，字体句柄 {}，系统错误 {}）。",
                    sent.0,
                    handle,
                    windows::core::Error::from_thread()
                );
            }
            let mut font = LOGFONTW::default();
            let count = unsafe {
                GetObjectW(
                    HGDIOBJ(handle as *mut _),
                    std::mem::size_of::<LOGFONTW>() as i32,
                    Some((&mut font as *mut LOGFONTW).cast()),
                )
            };
            if count != std::mem::size_of::<LOGFONTW>() as i32 {
                anyhow::bail!("无法读取桌面正在显示的字体。");
            }
            let end = font.lfFaceName.iter().position(|c| *c == 0).unwrap_or(32);
            return Ok(
                serde_json::json!({"dpi":unsafe { GetDpiForWindow(list) }, "height":font.lfHeight, "family":String::from_utf16_lossy(&font.lfFaceName[..end]), "weight":font.lfWeight}),
            );
        }
    }
    anyhow::bail!("未找到桌面图标视图。");
}
pub fn set_labels_hidden(hidden: bool) -> Result<()> {
    let (_a, v) = view()?;
    let mask = FWF_HIDEFILENAMES.0 as u32;
    unsafe {
        v.SetCurrentFolderFlags(mask, if hidden { mask } else { 0 })?;
        if (v.GetCurrentFolderFlags()? & mask != 0) != hidden {
            anyhow::bail!("桌面未接受图标文字设置。");
        }
    }
    Ok(())
}
