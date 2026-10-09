//! Read the actual native folder/file menu in an isolated diagnostic process.
use anyhow::Result;
use std::{fs, path::Path};
use windows::{
    Win32::{
        System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
        UI::{
            Shell::{BHID_SFUIObject, IContextMenu, IShellItem, SHCreateItemFromParsingName},
            WindowsAndMessaging::{
                CreatePopupMenu, DestroyMenu, GetMenuItemCount, GetMenuStringW, GetSubMenu, HMENU,
                MF_BYPOSITION,
            },
        },
    },
    core::PCWSTR,
};
struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct Menu(HMENU);
impl Drop for Menu {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyMenu(self.0);
        }
    }
}
fn names(menu: HMENU, depth: u32) -> Vec<serde_json::Value> {
    if depth > 5 {
        return vec![];
    }
    let mut entries = vec![];
    for index in 0..unsafe { GetMenuItemCount(Some(menu)) }.max(0) {
        let mut text = [0u16; 1024];
        let len = unsafe { GetMenuStringW(menu, index as u32, Some(&mut text), MF_BYPOSITION) };
        if len <= 0 {
            continue;
        }
        let submenu = unsafe { GetSubMenu(menu, index) };
        entries.push(
            serde_json::json!({"label":String::from_utf16_lossy(&text[..len as usize]),
            "children":if submenu.is_invalid() { vec![] } else { names(submenu, depth+1) }}),
        );
    }
    entries
}
fn query(path: &Path) -> Result<Vec<serde_json::Value>> {
    let wide: Vec<_> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain([0])
        .collect();
    let item: IShellItem = unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None) }?;
    let context: IContextMenu = unsafe { item.BindToHandler(None, &BHID_SFUIObject) }?;
    let menu = Menu(unsafe { CreatePopupMenu() }?);
    unsafe { context.QueryContextMenu(menu.0, 0, 1, 0x7fff, 0) }.ok()?;
    Ok(names(menu.0, 0))
}
pub fn run() -> Result<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok()?;
    let _com = Com;
    let root = crate::font_engine::data_root()?;
    let fixtures = root.join("menu-diagnostic-fixtures");
    fs::create_dir_all(fixtures.join("folder"))?;
    fs::write(fixtures.join("file.txt"), b"WinGlow menu probe")?;
    // A valid empty ZIP, so archive handlers can offer their real extraction verbs.
    fs::write(
        fixtures.join("archive.zip"),
        [
            0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    )?;
    let mut report = serde_json::Map::new();
    for (name, path) in [
        ("folder", fixtures.join("folder")),
        ("file", fixtures.join("file.txt")),
        ("archive", fixtures.join("archive.zip")),
    ] {
        report.insert(
            name.into(),
            match query(&path) {
                Ok(entries) => serde_json::json!({"items":entries}),
                Err(e) => serde_json::json!({"error":format!("{e:#}")}),
            },
        );
    }
    fs::write(
        root.join("native-menu-verification.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
