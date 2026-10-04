//! Enumerate registrations without loading third-party shell extensions.
use crate::{
    models::MenuItem,
    registry::{self, Entry, Scope},
    transaction::ValueStore,
};
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use windows::{Win32::UI::Shell::SHLoadIndirectString, core::PCWSTR};
use winreg::{
    RegKey,
    enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY},
};

const ROOTS: &[(&str, &str)] = &[
    ("*", "所有文件"),
    ("AllFilesystemObjects", "文件与文件夹"),
    ("Directory", "文件夹"),
    ("Folder", "文件夹"),
    ("Directory\\Background", "文件夹空白处"),
    ("Drive", "磁盘"),
    ("DesktopBackground", "桌面"),
    ("exefile", "EXE 程序"),
    ("lnkfile", "快捷方式"),
    ("Launcher.ImmersiveApplication", "应用快捷方式"),
    ("Unknown", "未知格式"),
    ("LibraryFolder", "库"),
    ("UserLibraryFolder", "库"),
    ("LibraryFolder\\Background", "库空白处"),
    ("CLSID\\{20D04FE0-3AEA-1069-A2D8-08002B30309D}", "此电脑"),
    ("CLSID\\{645FF040-5081-101B-9F08-00AA002F954E}", "回收站"),
    ("txtfile", "文本"),
    ("SystemFileAssociations\\.txt", "文本"),
    ("SystemFileAssociations\\image", "图片"),
    ("SystemFileAssociations\\.jpg", "图片"),
    ("SystemFileAssociations\\.jpeg", "图片"),
    ("SystemFileAssociations\\.png", "图片"),
    ("jpegfile", "图片"),
    ("pngfile", "图片"),
    ("SystemFileAssociations\\.pdf", "PDF"),
    ("CompressedFolder", "压缩文件"),
    ("SystemFileAssociations\\.zip", "压缩文件"),
    ("SystemFileAssociations\\.7z", "压缩文件"),
    ("SystemFileAssociations\\audio", "音视频"),
    ("SystemFileAssociations\\video", "音视频"),
];
const PROTECTED: &[&str] = &[
    "open",
    "opennewwindow",
    "explore",
    "runas",
    "runasuser",
    "delete",
    "rename",
    "properties",
    "printto",
];
const FILE_TYPES: &[(&str, &str)] = &[
    (".jpg", "图片"),
    (".jpeg", "图片"),
    (".png", "图片"),
    (".gif", "图片"),
    (".bmp", "图片"),
    (".webp", "图片"),
    (".txt", "文本"),
    (".md", "文本"),
    (".pdf", "PDF"),
    (".zip", "压缩文件"),
    (".7z", "压缩文件"),
    (".rar", "压缩文件"),
    (".mp3", "音视频"),
    (".wav", "音视频"),
    (".mp4", "音视频"),
    (".mkv", "音视频"),
];
fn valid_association_class(class: &str) -> bool {
    // A single Classes root only: no nested registry paths. LegacyDisable remains
    // the sole writable value and essential open/runas/delete verbs stay protected.
    !class.is_empty()
        && class.len() <= 200
        && class
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        && (class.contains('.')
            || class.to_ascii_lowercase().ends_with("file")
            || class.starts_with("AppX"))
        && class != "."
        && class != ".."
}
fn registration_roots() -> Vec<(String, String)> {
    let mut roots: Vec<_> = ROOTS
        .iter()
        .map(|(p, group)| (p.to_string(), group.to_string()))
        .collect();
    for (extension, group) in FILE_TYPES {
        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
        let selected = RegKey::predef(HKEY_CURRENT_USER).open_subkey(format!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\{extension}\\UserChoice")).ok().and_then(|k| k.get_value::<String, _>("ProgId").ok());
        let registered = hkcr
            .open_subkey(extension)
            .ok()
            .and_then(|k| k.get_value::<String, _>("").ok());
        let association = selected
            .filter(|s| valid_association_class(s))
            .or_else(|| registered.filter(|s| valid_association_class(s)));
        let shared = format!("SystemFileAssociations\\{extension}");
        if !roots.iter().any(|(p, _)| p == &shared) {
            roots.push((shared, group.to_string()));
        }
        for class in [Some(extension.to_string()), association]
            .into_iter()
            .flatten()
        {
            if valid_association_class(&class)
                && !roots
                    .iter()
                    .any(|(p, g)| p.eq_ignore_ascii_case(&class) && g == group)
            {
                roots.push((class, group.to_string()));
            }
        }
    }
    roots
}
pub fn valid_command_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 200
        && !name.contains(['\\', '/', '\0'])
        && !PROTECTED.iter().any(|p| {
            name.eq_ignore_ascii_case(p) || name.to_ascii_lowercase().ends_with(&format!(".{p}"))
        })
}
pub fn valid_verb_path(path: &str) -> bool {
    let static_root = ROOTS.iter().any(|(root, _)| {
        path.strip_prefix(&format!("{root}\\shell\\"))
            .is_some_and(|tail| {
                let parts: Vec<_> = tail.split("\\shell\\").collect();
                parts.len() <= 5 && parts.iter().all(|name| valid_command_name(name))
            })
    });
    static_root
        || path.split_once("\\shell\\").is_some_and(|(class, tail)| {
            (valid_association_class(class)
                || class
                    .strip_prefix("SystemFileAssociations\\")
                    .is_some_and(|e| FILE_TYPES.iter().any(|(extension, _)| e == *extension)))
                && {
                    let parts: Vec<_> = tail.split("\\shell\\").collect();
                    parts.len() <= 5 && parts.iter().all(|name| valid_command_name(name))
                }
        })
}
pub(crate) fn readable(raw: String) -> String {
    if raw.starts_with('@') {
        let wide: Vec<_> = raw.encode_utf16().chain([0]).collect();
        let mut out = [0u16; 512];
        if unsafe { SHLoadIndirectString(PCWSTR(wide.as_ptr()), &mut out, None) }.is_ok() {
            return String::from_utf16_lossy(
                &out[..out.iter().position(|c| *c == 0).unwrap_or(out.len())],
            );
        }
    }
    raw.trim().to_string()
}
fn friendly(raw: &str) -> String {
    let mut clean = raw.trim().to_string();
    // Windows accelerator labels are useful in the real menu, not in this list.
    for marker in ["(&", "（&"] {
        while let Some(start) = clean.find(marker) {
            let rest = &clean[start..];
            let Some(end) = rest.find([')', '）']) else {
                break;
            };
            clean.replace_range(
                start..start + end + rest[end..].chars().next().unwrap().len_utf8(),
                "",
            );
        }
    }
    clean = clean.replace('&', "");
    match clean.to_ascii_lowercase().as_str() {
        "copy as path menu" | "copyaspath" => "复制文件路径".into(),
        "doubao context menu" => "豆包".into(),
        "360zip file type" | "360zip" => "360 压缩".into(),
        "encryption context menu" => "加密 / 解密".into(),
        "previous versions property page" => "以前的版本".into(),
        "sharing" | "sharing handler" => "共享".into(),
        "pinto start screen" | "pintostartscreen" => "固定到开始菜单".into(),
        "sendto" | "microsoft sendto service" => "发送到".into(),
        "new menu handler" => "新建".into(),
        "slideshowcontextmenu" => "幻灯片菜单".into(),
        "enhanced storage context menu handler class" => "增强存储设备菜单".into(),
        "nvidia cpl context menu extension" => "NVIDIA 控制面板".into(),
        "find" => "搜索".into(),
        "print" => "打印".into(),
        "baidunetdisk" => "百度网盘".into(),
        "quarkclouddrive ai context menu" => "夸克网盘".into(),
        "open git bash here" => "在此打开 Git Bash".into(),
        "open git gui here" => "在此打开 Git GUI".into(),
        ".spotlightlearnmore" => "了解此图片".into(),
        ".spotlightnextimage" => "切换聚焦图片".into(),
        "editstickers" => "编辑桌面贴纸".into(),
        "open with" | "openwith" => "打开方式".into(),
        _ => clean,
    }
}
fn extension_label(raw: &str, server: Option<&RegKey>) -> String {
    if uuid::Uuid::parse_str(raw.trim_matches(['{', '}'])).is_ok() {
        return "未命名程序扩展".into();
    }
    let path = server
        .and_then(|key| key.get_value::<String, _>("").ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match (raw, path.rsplit('\\').next().unwrap_or("")) {
        ("DesktopContext Class", "nvui.dll") => "NVIDIA 控制面板".into(),
        ("CompatContextMenu Class", "acppage.dll") => "程序兼容性".into(),
        ("FileSyncEx", "filesyncshell64.dll") => "OneDrive".into(),
        ("EPP", "shellext.dll") if path.contains("\\windows defender\\") => {
            "Microsoft Defender 扫描".into()
        }
        _ => friendly(raw),
    }
}
fn targets(group: &str) -> Vec<String> {
    match group {
        "所有文件" => [
            "所有文件",
            "EXE 程序",
            "图片",
            "文本",
            "PDF",
            "压缩文件",
            "音视频",
            "快捷方式",
        ]
        .iter()
        .map(|s| (*s).into())
        .collect(),
        "文件与文件夹" => {
            let mut values = targets("所有文件");
            values.push("文件夹".into());
            values
        }
        "文件夹空白处" => vec!["桌面".into(), "文件夹空白处".into()],
        _ => vec![group.into()],
    }
}
fn context_group(class: &str, group: &str) -> String {
    match class {
        "Directory\\Background" => "桌面 / 文件夹空白处",
        "SystemFileAssociations\\.jpg" | "jpegfile" => "JPG 图片",
        "SystemFileAssociations\\.jpeg" => "JPEG 图片",
        "SystemFileAssociations\\.png" | "pngfile" => "PNG 图片",
        "SystemFileAssociations\\.zip" | "CompressedFolder" => "ZIP 压缩文件",
        "SystemFileAssociations\\.7z" => "7Z 压缩文件",
        "SystemFileAssociations\\audio" => "音频文件",
        "SystemFileAssociations\\video" => "视频文件",
        _ => group,
    }
    .into()
}
fn child_items(
    key: &RegKey,
    parent: &registry::Slot,
    group: &str,
    icons: bool,
    depth: usize,
    slots: &mut Vec<(MenuItem, registry::Slot)>,
) -> Result<Vec<MenuItem>> {
    if depth >= 4 {
        return Ok(Vec::new());
    }
    let mut sources = Vec::new();
    if let Scope::MenuVerb { machine, path } = &parent.scope
        && let Ok(shell) = key.open_subkey_with_flags("shell", KEY_READ | KEY_WOW64_64KEY)
    {
        for name in shell.enum_keys().flatten().take(100) {
            let child_path = format!("{path}\\shell\\{name}");
            if valid_verb_path(&child_path)
                && let Ok(child) = shell.open_subkey_with_flags(&name, KEY_READ | KEY_WOW64_64KEY)
            {
                sources.push((
                    name,
                    child,
                    registry::slot(
                        Scope::MenuVerb {
                            machine: *machine,
                            path: child_path,
                        },
                        "LegacyDisable",
                    ),
                ));
            }
        }
    }
    if let Ok(commands) = key.get_value::<String, _>("SubCommands") {
        let root = RegKey::predef(HKEY_LOCAL_MACHINE);
        for name in commands
            .split(';')
            .filter(|s| valid_command_name(s))
            .take(100)
        {
            let scope = Scope::CommandStoreVerb { name: name.into() };
            if let Ok(child) = root.open_subkey_with_flags(scope.path(), KEY_READ | KEY_WOW64_64KEY)
            {
                sources.push((name.into(), child, registry::slot(scope, "LegacyDisable")));
            }
        }
    }
    let mut result = Vec::new();
    for (name, child, slot) in sources {
        if child.get_raw_value("ProgrammaticAccessOnly").is_ok() {
            continue;
        }
        let label = readable(
            child
                .get_value::<String, _>("MUIVerb")
                .or_else(|_| child.get_value(""))
                .unwrap_or(name.clone()),
        );
        let sub_items = child_items(&child, &slot, group, icons, depth + 1, slots)?;
        let (icon_data_url, icon_source) = item_icon(&child, None, icons);
        let item = MenuItem {
            id: id(&slot.scope, &name),
            label: friendly(&label),
            raw_label: label.clone(),
            group: group.into(),
            targets: targets(group),
            enabled: child.get_raw_value("LegacyDisable").is_err(),
            auto_hide: crate::menu_policy::should_hide(&label, &slot, &child, None),
            kind: "子菜单".into(),
            menu_level: if sub_items.is_empty() {
                "direct"
            } else {
                "cascade"
            }
            .into(),
            children: sub_items.iter().map(|i| i.label.clone()).collect(),
            sub_items,
            visibility_note: None,
            icon_data_url,
            icon_source,
        };
        slots.push((item.clone(), slot));
        result.push(item);
    }
    Ok(result)
}
fn item_icon(
    key: &RegKey,
    server: Option<&RegKey>,
    icons: bool,
) -> (Option<String>, Option<String>) {
    if !icons {
        return (None, None);
    }
    for value in ["Icon", "DefaultIcon"] {
        if let Ok(raw) = key.get_value::<String, _>(value)
            && let Some(icon) = crate::menu_icon::extract(&raw)
        {
            return (Some(icon), Some("菜单图标".into()));
        }
    }
    if let Ok(default_icon) = key.open_subkey_with_flags("DefaultIcon", KEY_READ | KEY_WOW64_64KEY)
        && let Ok(raw) = default_icon.get_value::<String, _>("")
        && let Some(icon) = crate::menu_icon::extract(&raw)
    {
        return (Some(icon), Some("程序图标".into()));
    }
    if let Some(server) = server
        && let Ok(raw) = server.get_value::<String, _>("")
        && let Some(icon) = crate::menu_icon::extract(&raw)
    {
        return (Some(icon), Some("程序图标".into()));
    }
    if let Ok(command) = key.open_subkey_with_flags("command", KEY_READ | KEY_WOW64_64KEY)
        && let Ok(raw) = command.get_value::<String, _>("")
        && let Some(path) = crate::menu_icon::command_executable(&raw)
        && let Some(icon) = crate::menu_icon::extract(&path)
    {
        return (Some(icon), Some("程序图标".into()));
    }
    (None, None)
}
fn id(scope: &Scope, name: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{scope:?}:{name}").as_bytes())
    )
}
pub fn scan() -> Result<Vec<(MenuItem, registry::Slot)>> {
    let mut grouped: Vec<(MenuItem, registry::Slot)> = Vec::new();
    for (item, slot) in scan_impl(true)? {
        if item.kind == "子菜单" {
            continue;
        }
        if let Some((existing, _)) = grouped.iter_mut().find(|(old, _)| old.id == item.id) {
            existing.enabled |= item.enabled;
            existing.auto_hide &= item.auto_hide;
            for target in item.targets {
                if !existing.targets.contains(&target) {
                    existing.targets.push(target);
                }
            }
            if existing.group != item.group {
                existing.group = existing.targets.join(" / ");
            }
        } else {
            grouped.push((item, slot));
        }
    }
    Ok(grouped)
}
// Merge only leaf verbs with matching registration names and executable behavior.
// Equal translated titles alone do not establish that two commands are the same.
fn leaf_identity(name: &str, label: &str, command: &str, delegate: &str) -> Option<String> {
    if command.is_empty() && delegate.is_empty() {
        return None;
    }
    Some(format!(
        "merged:{:x}",
        Sha256::digest(
            format!(
                "{}\0{label}\0{command}\0{}",
                name.to_lowercase(),
                delegate.to_lowercase()
            )
            .as_bytes()
        )
    ))
}
pub(crate) fn scan_impl(icons: bool) -> Result<Vec<(MenuItem, registry::Slot)>> {
    let mut items: Vec<(MenuItem, registry::Slot)> = Vec::new();
    let mut child_slots = Vec::new();
    let roots = registration_roots();
    for machine in [false, true] {
        let root = RegKey::predef(if machine {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        });
        for (class, group) in &roots {
            let base = format!("Software\\Classes\\{class}");
            let verbs = match root
                .open_subkey_with_flags(format!("{base}\\shell"), KEY_READ | KEY_WOW64_64KEY)
            {
                Ok(v) => Some(v),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            };
            if let Some(verbs) = verbs {
                for name in verbs.enum_keys() {
                    let name = name?;
                    let path = format!("{class}\\shell\\{name}");
                    if !valid_verb_path(&path) {
                        continue;
                    }
                    let key = verbs.open_subkey_with_flags(&name, KEY_READ | KEY_WOW64_64KEY)?;
                    // Do not expose programmatic-only verbs. Cascades are switched as a group.
                    if key.get_raw_value("ProgrammaticAccessOnly").is_ok() {
                        continue;
                    }
                    let scope = Scope::MenuVerb { machine, path };
                    let slot = registry::slot(scope, "LegacyDisable");
                    let label = readable(
                        key.get_value::<String, _>("MUIVerb")
                            .or_else(|_| key.get_value(""))
                            .unwrap_or(name.clone()),
                    );
                    let enabled = crate::registry::WindowsRegistry.read(&slot)?.is_none();
                    let sub_items = child_items(&key, &slot, group, icons, 0, &mut child_slots)?;
                    let children: Vec<String> = sub_items.iter().map(|i| i.label.clone()).collect();
                    let cascading = !children.is_empty()
                        || key.get_raw_value("SubCommands").is_ok()
                        || key.get_raw_value("ExtendedSubCommandsKey").is_ok();
                    let (icon_data_url, icon_source) = item_icon(&key, None, icons);
                    let command_key = key.open_subkey("command").ok();
                    let command = command_key
                        .as_ref()
                        .and_then(|k| k.get_value::<String, _>("").ok())
                        .unwrap_or_default();
                    let delegate = command_key
                        .as_ref()
                        .and_then(|k| k.get_value::<String, _>("DelegateExecute").ok())
                        .or_else(|| key.get_value("ExplorerCommandHandler").ok())
                        .unwrap_or_default();
                    let identity = if !cascading {
                        leaf_identity(&name, &friendly(&label), &command, &delegate)
                    } else {
                        None
                    };
                    items.push((
                        MenuItem {
                            id: identity.unwrap_or_else(|| id(&slot.scope, &name)),
                            label: friendly(&label),
                            auto_hide: crate::menu_policy::should_hide(&label, &slot, &key, None),
                            raw_label: label,
                            targets: targets(group),
                            menu_level: if cascading { "cascade" } else { "direct" }.into(),
                            children,
                            sub_items,
                            visibility_note: if key.get_raw_value("Extended").is_ok() {
                                Some("按住 Shift 再右键才显示。".into())
                            } else if key.get_raw_value("AppliesTo").is_ok() {
                                Some("只在符合文件条件时显示。".into())
                            } else {
                                None
                            },
                            icon_data_url,
                            icon_source,
                            group: context_group(class, group),
                            enabled,
                            kind: if machine {
                                "所有用户"
                            } else {
                                "当前用户"
                            }
                            .into(),
                        },
                        slot,
                    ));
                }
            }
            let handlers = match root.open_subkey_with_flags(
                format!("{base}\\shellex\\ContextMenuHandlers"),
                KEY_READ | KEY_WOW64_64KEY,
            ) {
                Ok(v) => Some(v),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            };
            if let Some(handlers) = handlers {
                for name in handlers.enum_keys() {
                    let name = name?;
                    let key = handlers.open_subkey_with_flags(&name, KEY_READ | KEY_WOW64_64KEY)?;
                    let guid = key.get_value::<String, _>("").unwrap_or(name.clone());
                    if uuid::Uuid::parse_str(guid.trim_matches(['{', '}'])).is_err() {
                        continue;
                    }
                    let slot = registry::slot(Scope::BlockedExtensions, guid.clone());
                    // One CLSID may appear in several categories; expose a single global switch.
                    if let Some((item, _)) = items.iter_mut().find(|(_, s)| s == &slot) {
                        for target in targets(group) {
                            if !item.targets.contains(&target) {
                                item.targets.push(target);
                            }
                        }
                        continue;
                    }
                    // CLSID metadata is merged for the current user; the registration
                    // itself may live in a different hive from its handler reference.
                    let clsid = RegKey::predef(HKEY_CLASSES_ROOT)
                        .open_subkey_with_flags(
                            format!("CLSID\\{guid}"),
                            KEY_READ | KEY_WOW64_64KEY,
                        )
                        .ok();
                    let server = clsid.as_ref().and_then(|key| {
                        key.open_subkey_with_flags("InprocServer32", KEY_READ | KEY_WOW64_64KEY)
                            .ok()
                    });
                    let (mut icon_data_url, mut icon_source) = clsid
                        .as_ref()
                        .map(|key| item_icon(key, server.as_ref(), icons))
                        .unwrap_or((None, None));
                    let label = readable(
                        clsid
                            .as_ref()
                            .and_then(|k| k.get_value::<String, _>("").ok())
                            .filter(|s| !s.is_empty())
                            .unwrap_or(name),
                    );
                    let server_path = server
                        .as_ref()
                        .and_then(|key| key.get_value::<String, _>("").ok());
                    let dictionary = crate::menu_dictionary::lookup(&guid);
                    let display_label = dictionary
                        .as_ref()
                        .and_then(|info| info.label(server_path.as_deref()))
                        .map(|s| friendly(&s))
                        .unwrap_or_else(|| extension_label(&label, server.as_ref()));
                    if icons
                        && let Some(icon) = dictionary
                            .as_ref()
                            .and_then(|info| info.icon(server_path.as_deref()))
                    {
                        icon_data_url = Some(icon);
                        icon_source = Some("菜单识别库".into());
                    }
                    let enabled = crate::registry::WindowsRegistry.read(&slot)?.is_none()
                        && crate::registry::WindowsRegistry
                            .read(&registry::slot(Scope::MachineBlockedExtensions, &guid))?
                            .is_none();
                    items.push((
                        MenuItem {
                            id: id(&slot.scope, &guid),
                            label: display_label,
                            auto_hide: crate::menu_policy::should_hide(
                                &label,
                                &slot,
                                &key,
                                server.as_ref(),
                            ),
                            raw_label: label,
                            targets: targets(group),
                            menu_level: "extension".into(),
                            children: vec![],
                            sub_items: vec![],
                            visibility_note: Some("显示哪些项目由程序和所选文件决定。".into()),
                            icon_data_url,
                            icon_source,
                            group: "扩展菜单".into(),
                            enabled,
                            kind: "当前用户".into(),
                        },
                        slot,
                    ));
                }
            }
        }
    }
    items.sort_by(|a, b| {
        (&a.0.group, &a.0.label, &a.0.kind).cmp(&(&b.0.group, &b.0.label, &b.0.kind))
    });
    items.extend(child_slots);
    Ok(items)
}
pub fn resolve(id: &str, enabled: bool) -> Result<Vec<Entry>> {
    let slots: Vec<_> = scan_impl(false)?
        .into_iter()
        .filter(|(item, _)| item.id == id)
        .map(|(_, slot)| slot)
        .collect();
    if slots.is_empty() {
        bail!("菜单项已变化，请刷新后再试。");
    }
    let mut entries: Vec<_> = slots
        .into_iter()
        .map(|slot| Entry {
            slot,
            value: if enabled {
                None
            } else {
                Some(registry::string(""))
            },
        })
        .collect();
    if enabled && entries[0].slot.scope == Scope::BlockedExtensions {
        let machine = registry::slot(Scope::MachineBlockedExtensions, &entries[0].slot.name);
        if crate::registry::WindowsRegistry.read(&machine)?.is_some() {
            entries.push(Entry {
                slot: machine,
                value: None,
            });
        }
    }
    Ok(entries)
}
pub fn needs_admin(id: &str) -> Result<bool> {
    Ok(resolve(id, true)?.iter().any(|e| {
        matches!(
            e.slot.scope,
            Scope::MenuVerb { machine: true, .. }
                | Scope::CommandStoreVerb { .. }
                | Scope::MachineBlockedExtensions
        )
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn association_paths_are_limited_to_leaf_classes_and_keep_core_verbs_protected() {
        assert!(valid_verb_path("Acrobat.Document.DC\\shell\\custom"));
        assert!(valid_verb_path("AppX123abc\\shell\\share"));
        assert!(!valid_verb_path("Acrobat.Document.DC\\shell\\open"));
        assert!(!valid_verb_path(
            "Software\\Classes\\Acrobat.Document.DC\\shell\\custom"
        ));
        assert!(!valid_verb_path("..\\shell\\custom"));
        assert!(!valid_verb_path(
            "Acrobat.Document.DC\\shell\\custom\\command"
        ));
    }
    #[test]
    fn duplicate_titles_require_same_command_identity() {
        let jpg = leaf_identity("setdesktopwallpaper", "设置为桌面背景", "wallpaper %1", "");
        assert_eq!(
            jpg,
            leaf_identity("SetDesktopWallpaper", "设置为桌面背景", "wallpaper %1", "")
        );
        assert_ne!(
            jpg,
            leaf_identity("setdesktopwallpaper", "设置为桌面背景", "other %1", "")
        );
        assert_eq!(None, leaf_identity("unknown", "相同标题", "", ""));
    }
    #[test]
    fn static_children_are_real_independent_nested_switches() {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let path = format!(
            r"Software\WinGlow\TransactionTests\{}",
            uuid::Uuid::new_v4()
        );
        let key = root.create_subkey(&path).unwrap().0;
        let upload = key.create_subkey(r"shell\Upload").unwrap().0;
        upload.set_value("MUIVerb", &"上传到夸克网盘").unwrap();
        upload.set_value("LegacyDisable", &"").unwrap();
        key.create_subkey(r"shell\open").unwrap();
        let nested = key.create_subkey(r"shell\Tools\shell\Inspect").unwrap().0;
        nested.set_value("MUIVerb", &"检查").unwrap();
        let parent = registry::slot(
            Scope::MenuVerb {
                machine: false,
                path: r"Directory\shell\Fixture".into(),
            },
            "LegacyDisable",
        );
        let mut slots = Vec::new();
        let items = child_items(&key, &parent, "文件夹", false, 0, &mut slots).unwrap();
        assert_eq!(items.len(), 2);
        let upload = items.iter().find(|i| i.label == "上传到夸克网盘").unwrap();
        assert!(!upload.enabled);
        assert!(upload.auto_hide);
        let tools = items.iter().find(|i| i.label == "Tools").unwrap();
        assert_eq!(tools.sub_items[0].label, "检查");
        assert_ne!(tools.id, tools.sub_items[0].id);
        assert_eq!(slots.len(), 3);
        assert!(slots.iter().any(|(_, slot)| matches!(&slot.scope, Scope::MenuVerb { path, .. } if path == r"Directory\shell\Fixture\shell\Tools\shell\Inspect")));
        assert!(path.starts_with(r"Software\WinGlow\TransactionTests\"));
        root.delete_subkey_all(&path).unwrap();
    }
    #[test]
    fn nested_paths_and_shared_commands_remain_constrained() {
        assert!(valid_verb_path("Directory\\shell\\Tools\\shell\\Upload"));
        for path in [
            "Directory\\shell\\Tools\\shell\\open",
            "Directory\\shell\\Tools\\command",
            "Directory\\shell\\Tools\\shell\\x\\CLSID",
            "Directory\\shell\\a\\shell\\b\\shell\\c\\shell\\d\\shell\\e\\shell\\f",
        ] {
            assert!(!valid_verb_path(path), "{path}");
        }
        assert!(valid_command_name("vendor.Upload"));
        assert!(!valid_command_name("Windows.open"));
        assert!(!valid_command_name("evil\\command"));
    }
    #[test]
    fn paths_cannot_escape_menu_or_disable_open() {
        assert!(valid_verb_path("Directory\\Background\\shell\\GitHere"));
        assert!(!valid_verb_path("Directory\\shell\\open"));
        assert!(!valid_verb_path("*\\shell\\a\\command"));
        assert!(!valid_verb_path("CLSID\\shell\\x"));
        assert!(valid_verb_path("exefile\\shell\\scan"));
        assert!(!valid_verb_path("exefile\\shell\\runas"));
        assert!(valid_verb_path("SystemFileAssociations\\.png\\shell\\edit"));
    }
    #[test]
    fn file_filters_include_inherited_menu_items() {
        assert!(targets("所有文件").contains(&"EXE 程序".into()));
        assert!(targets("文件与文件夹").contains(&"文件夹".into()));
        assert!(!targets("文件夹空白处").contains(&"文件夹".into()));
        assert!(targets("文件夹空白处").contains(&"桌面".into()));
        assert_eq!(friendly("Copy as Path Menu"), "复制文件路径");
        assert_eq!(friendly("Unknown extension"), "Unknown extension");
        assert_eq!(friendly("设置为桌面背景(&B)"), "设置为桌面背景");
        assert_eq!(friendly("打开（&O）"), "打开");
    }
}
