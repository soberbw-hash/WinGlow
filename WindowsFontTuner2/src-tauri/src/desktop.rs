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
