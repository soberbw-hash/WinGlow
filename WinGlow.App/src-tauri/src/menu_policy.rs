//! Deterministic provenance-based cleanup. Unknown entries remain enabled.
use crate::{
    menu,
    registry::{self, Entry, Scope, Slot},
};
use anyhow::Result;
use windows::{
    Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
    core::PCWSTR,
};
use winreg::RegKey;

fn company(source: &str) -> Option<String> {
    let trimmed = source.trim();
    let candidate = if let Some(rest) = trimmed.strip_prefix('"') {
        rest.split('"').next()?
    } else {
        let lower = trimmed.to_ascii_lowercase();
        let end = [".exe", ".dll"]
            .iter()
            .filter_map(|ext| lower.find(ext).map(|i| i + 4))
            .min()?;
        &trimmed[..end]
    };
    let path = std::path::Path::new(candidate);
    if !path.is_absolute() || candidate.starts_with("\\\\") || !path.is_file() {
        return None;
    }
    let wide: Vec<u16> = candidate.encode_utf16().chain([0]).collect();
    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None) };
    if size == 0 || size > 1_000_000 {
        return None;
    }
    let mut bytes = vec![0u8; size as usize];
    unsafe { GetFileVersionInfoW(PCWSTR(wide.as_ptr()), None, size, bytes.as_mut_ptr().cast()) }
        .ok()?;
    let query = |key: &str| -> Option<(*mut std::ffi::c_void, u32)> {
        let name: Vec<u16> = key.encode_utf16().chain([0]).collect();
        let mut ptr = std::ptr::null_mut();
        let mut length = 0;
        unsafe {
            VerQueryValueW(
                bytes.as_ptr().cast(),
                PCWSTR(name.as_ptr()),
                &mut ptr,
                &mut length,
            )
        }
        .as_bool()
        .then_some((ptr, length))
    };
    let (translations, len) = query("\\VarFileInfo\\Translation")?;
    if len < 4 || translations.is_null() {
        return None;
    }
    let language = unsafe { std::ptr::read_unaligned(translations.cast::<u16>()) };
    let codepage = unsafe { std::ptr::read_unaligned(translations.cast::<u16>().add(1)) };
    let (text, len) = query(&format!(
        "\\StringFileInfo\\{language:04x}{codepage:04x}\\CompanyName"
    ))?;
    if text.is_null() || !(2..=512).contains(&len) {
        return None;
    }
    let value = unsafe { std::slice::from_raw_parts(text.cast::<u16>(), len as usize) };
    String::from_utf16(&value[..value.iter().position(|c| *c == 0).unwrap_or(value.len())])
        .ok()
        .filter(|s| !s.trim().is_empty())
}

fn critical(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "defender",
        "security",
        "antivirus",
        "加密",
        "解密",
        "安全",
        "扫描",
        "openwith",
        "open with",
        "sendto",
        "send to",
        "new menu",
        "sharing",
        "compat",
        "copy as path",
        "pinto",
        "file ownership",
    ]
    .iter()
    .any(|x| n.contains(x))
}
fn confirmed_third_party(name: &str, source: &str) -> bool {
    if critical(name) || critical(source) {
        return false;
    }
    let path = source.trim_matches('"').to_lowercase().replace('/', "\\");
    let windows = std::env::var("WINDIR")
        .unwrap_or_else(|_| "C:\\Windows".into())
        .to_lowercase();
    if path.starts_with(&(windows + "\\"))
        || [
            "\\microsoft\\",
            "\\windowsapps\\",
            "\\windows defender\\",
            "\\onedrive\\",
        ]
        .iter()
        .any(|x| path.contains(x))
    {
        return false;
    }
    // Read version resources without loading any extension. This is ownership evidence,
    // not a signature validation or a claim that the publisher is trustworthy.
    if let Some(owner) = company(source) {
        let owner = owner.to_lowercase();
        return !owner.contains("microsoft")
            && !critical(&owner)
            && ![
                "eset",
                "avast",
                "avira",
                "kaspersky",
                "bitdefender",
                "symantec",
                "norton",
                "malwarebytes",
                "mcafee",
                "huorong",
                "火绒",
            ]
            .iter()
            .any(|x| owner.contains(x));
    }
    // Recognized products are a fallback when a resource has no company metadata.
    let product = format!("{} {}", name.to_lowercase(), path);
    let known = [
        "360zip",
        "360 压缩",
        "7-zip",
        "7zip",
        "winrar",
        "git bash",
        "git gui",
        "githere",
        "git_gui",
        "git_shell",
        "\\git\\",
        "doubao",
        "豆包",
        "baidunetdisk",
        "百度网盘",
        "wallpaper",
        "armoury",
        "cyberpunk",
        "nvidia",
        "nvui.dll",
        "notepad++",
        "vscode",
        "code.exe",
        "everything",
        "potplayer",
        "foxit",
        "wps",
    ]
    .iter()
    .any(|x| product.contains(x));
    known && !path.is_empty() && (path.contains(".exe") || path.contains(".dll"))
}
pub fn should_hide(name: &str, slot: &Slot, key: &RegKey, server: Option<&RegKey>) -> bool {
    let source = match slot.scope {
        Scope::BlockedExtensions => server.and_then(|k| k.get_value::<String, _>("").ok()),
        _ => key
            .open_subkey("command")
            .ok()
            .and_then(|k| k.get_value::<String, _>("").ok()),
    }
    .unwrap_or_default();
    confirmed_third_party(name, &source)
}
pub fn plan() -> Result<Vec<Entry>> {
    Ok(menu::scan()?
        .into_iter()
        .filter(|(item, _)| item.enabled && item.auto_hide)
        .map(|(_, slot)| Entry {
            slot,
            value: Some(registry::string("")),
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_preserves_critical_and_unknown_entries() {
        assert!(!confirmed_third_party(
            "EPP",
            "C:\\Program Files\\Windows Defender\\shellext.dll"
        ));
        assert!(!confirmed_third_party(
            "New Menu",
            "C:\\Windows\\system32\\shell32.dll"
        ));
        assert!(!confirmed_third_party("Unknown", "D:\\Tools\\unknown.dll"));
        assert!(!confirmed_third_party(
            "7zip security scanner",
            "D:\\7zip\\scanner.exe"
        ));
        assert!(confirmed_third_party("7-Zip", "D:\\Apps\\7-Zip\\7-zip.dll"));
        assert!(confirmed_third_party(
            "Open Git Bash Here",
            "\"C:\\Program Files\\Git\\git-bash.exe\" --cd=\"%V\""
        ));
        assert!(!confirmed_third_party("Open Git Bash Here", ""));
        assert!(!confirmed_third_party(
            "Git Shell",
            "C:\\Windows\\system32\\git.dll"
        ));
    }
}
