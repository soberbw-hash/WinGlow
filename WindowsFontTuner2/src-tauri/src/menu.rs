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
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY},
};

const ROOTS: &[(&str, &str)] = &[
    ("*", "文件"),
    ("AllFilesystemObjects", "文件与文件夹"),
    ("Directory", "文件夹"),
    ("Directory\\Background", "文件夹空白处"),
    ("Drive", "磁盘"),
    ("DesktopBackground", "桌面"),
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
];
pub fn valid_verb_path(path: &str) -> bool {
    ROOTS.iter().any(|(root, _)| {
        path.strip_prefix(&format!("{root}\\shell\\"))
            .is_some_and(|name| {
                !name.is_empty()
                    && name.len() < 200
                    && !name.contains(['\\', '/', '\0'])
                    && !PROTECTED.iter().any(|p| name.eq_ignore_ascii_case(p))
            })
    })
}
fn readable(raw: String) -> String {
    if raw.starts_with('@') {
        let wide: Vec<_> = raw.encode_utf16().chain([0]).collect();
        let mut out = [0u16; 512];
        if unsafe { SHLoadIndirectString(PCWSTR(wide.as_ptr()), &mut out, None) }.is_ok() {
            return String::from_utf16_lossy(
                &out[..out.iter().position(|c| *c == 0).unwrap_or(out.len())],
            );
        }
    }
    raw
}
fn id(scope: &Scope, name: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{scope:?}:{name}").as_bytes())
    )
}
pub fn scan() -> Result<Vec<(MenuItem, registry::Slot)>> {
    let mut items = Vec::new();
    for machine in [false, true] {
        let root = RegKey::predef(if machine {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        });
        for (class, group) in ROOTS {
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
                    // Cascading and programmatic-only verbs need a different policy.
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
                    items.push((
                        MenuItem {
                            id: id(&slot.scope, &name),
                            label,
                            group: (*group).into(),
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
                    if items.iter().any(|(_, s)| s == &slot) {
                        continue;
                    }
                    let clsid = root
                        .open_subkey_with_flags(
                            format!("Software\\Classes\\CLSID\\{guid}"),
                            KEY_READ | KEY_WOW64_64KEY,
                        )
                        .ok();
                    let label = readable(
                        clsid
                            .and_then(|k| k.get_value::<String, _>("").ok())
                            .filter(|s| !s.is_empty())
                            .unwrap_or(name),
                    );
                    let enabled = crate::registry::WindowsRegistry.read(&slot)?.is_none()
                        && crate::registry::WindowsRegistry
                            .read(&registry::slot(Scope::MachineBlockedExtensions, &guid))?
                            .is_none();
                    items.push((
                        MenuItem {
                            id: id(&slot.scope, &guid),
                            label,
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
    Ok(items)
}
pub fn resolve(id: &str, enabled: bool) -> Result<Vec<Entry>> {
    let Some((_, slot)) = scan()?.into_iter().find(|(item, _)| item.id == id) else {
        bail!("菜单项已变化，请刷新后再试。");
    };
    let mut entries = vec![Entry {
        slot,
        value: if enabled {
            None
        } else {
            Some(registry::string(""))
        },
    }];
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
            Scope::MenuVerb { machine: true, .. } | Scope::MachineBlockedExtensions
        )
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_cannot_escape_menu_or_disable_open() {
        assert!(valid_verb_path("Directory\\Background\\shell\\GitHere"));
        assert!(!valid_verb_path("Directory\\shell\\open"));
        assert!(!valid_verb_path("*\\shell\\a\\command"));
        assert!(!valid_verb_path("CLSID\\shell\\x"));
    }
}
