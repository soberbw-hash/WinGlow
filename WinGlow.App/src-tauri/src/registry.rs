use crate::preset_data::{MANAGED_ALIASES, PRESETS};
use crate::transaction::ValueStore;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, KEY_WRITE, REG_BINARY,
    REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ, REG_NONE, REG_SZ,
};
use winreg::{RegKey, RegValue};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    Substitutes,
    Links,
    Fonts,
    SystemFonts,
    UserSystemFonts,
    Desktop,
    Avalon(String),
    Explorer,
    IconOverrides,
    DesktopView,
    MenuVerb { machine: bool, path: String },
    BlockedExtensions,
    MachineBlockedExtensions,
    Startup,
    WindowMetrics,
    LiveUiFonts,
    TaskbarConfig,
}

impl Scope {
    pub fn path(&self) -> String {
        match self {
            Self::Substitutes => {
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\FontSubstitutes".into()
            }
            Self::Links => {
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\FontLink\SystemLink".into()
            }
            Self::Fonts | Self::SystemFonts | Self::UserSystemFonts => {
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts".into()
            }
            Self::Desktop => r"Control Panel\Desktop".into(),
            Self::Avalon(display) => format!(r"Software\Microsoft\Avalon.Graphics\{display}"),
            Self::Explorer => r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced".into(),
            Self::IconOverrides => {
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Icons".into()
            }
            Self::DesktopView => r"Software\Microsoft\Windows\Shell\Bags\1\Desktop".into(),
            Self::MenuVerb { path, .. } => format!(r"Software\Classes\{path}"),
            Self::BlockedExtensions | Self::MachineBlockedExtensions => {
                r"Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked".into()
            }
            Self::Startup => r"Software\Microsoft\Windows\CurrentVersion\Run".into(),
            Self::WindowMetrics => r"Control Panel\Desktop\WindowMetrics".into(),
            Self::TaskbarConfig => String::new(),
            Self::LiveUiFonts => String::new(), // Virtual slot, handled through native APIs below.
        }
    }
    fn root(&self) -> RegKey {
        RegKey::predef(
            if matches!(
                self,
                Self::SystemFonts
                    | Self::MachineBlockedExtensions
                    | Self::Substitutes
                    | Self::Links
                    | Self::Fonts
                    | Self::MenuVerb { machine: true, .. }
            ) {
                HKEY_LOCAL_MACHINE
            } else {
                HKEY_CURRENT_USER
            },
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Slot {
    pub scope: Scope,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredValue {
    pub kind: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub slot: Slot,
    pub value: Option<StoredValue>,
}

pub fn slot(scope: Scope, name: impl Into<String>) -> Slot {
    Slot {
        scope,
        name: name.into(),
    }
}
pub fn string(value: &str) -> StoredValue {
    StoredValue {
        kind: 1,
        bytes: value
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect(),
    }
}
pub fn multi_string(values: &[String]) -> StoredValue {
    let mut bytes = Vec::new();
    for value in values {
        bytes.extend(string(value).bytes);
    }
    bytes.extend([0, 0]);
    StoredValue { kind: 7, bytes }
}
pub fn as_string(value: &StoredValue) -> Option<String> {
    if !matches!(value.kind, 1 | 2) || !value.bytes.len().is_multiple_of(2) {
        return None;
    }
    let chars: Vec<u16> = value
        .bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&c| c != 0)
        .collect();
    String::from_utf16(&chars).ok()
}

fn known_font_name(name: &str) -> bool {
    PRESETS.iter().any(|p| {
        let suffix = name.strip_prefix(p.family);
        suffix.is_some_and(|s| {
            matches!(
                s,
                "" | " Regular"
                    | " Medium"
                    | " Semibold"
                    | " SemiBold"
                    | " Bold"
                    | " Heavy"
                    | " Light"
                    | " Ultralight"
                    | " Thin"
            )
        })
    })
}

pub fn validate_slot(slot: &Slot) -> Result<()> {
    let allowed = match &slot.scope {
        Scope::Substitutes => MANAGED_ALIASES.contains(&slot.name.as_str()),
        Scope::Links => {
            MANAGED_ALIASES.contains(&slot.name.as_str()) || known_font_name(&slot.name)
        }
        Scope::Fonts => slot
            .name
            .strip_suffix(" (TrueType)")
            .or_else(|| slot.name.strip_suffix(" (OpenType)"))
            .is_some_and(known_font_name),
        Scope::SystemFonts | Scope::UserSystemFonts => crate::repair::SYSTEM_FONTS
            .iter()
            .any(|(name, _)| *name == slot.name),
        Scope::Desktop => [
            "FontSmoothing",
            "FontSmoothingType",
            "FontSmoothingGamma",
            "FontSmoothingOrientation",
        ]
        .contains(&slot.name.as_str()),
        Scope::Avalon(display) => {
            display.strip_prefix("DISPLAY").is_some_and(|s| {
                !s.is_empty() && s.len() < 10 && s.bytes().all(|b| b.is_ascii_digit())
            }) && [
                "PixelStructure",
                "GammaLevel",
                "ClearTypeLevel",
                "TextContrastLevel",
            ]
            .contains(&slot.name.as_str())
        }
        Scope::Explorer => matches!(
            slot.name.as_str(),
            "HideIcons" | "HideFileExt" | "TaskbarGlomLevel"
        ),
        Scope::IconOverrides => matches!(slot.name.as_str(), "29" | "77"),
        Scope::DesktopView => slot.name == "FFlags",
        Scope::WindowMetrics => crate::ui_fonts::NAMES.contains(&slot.name.as_str()),
        Scope::LiveUiFonts => slot.name == "Fonts",
        Scope::TaskbarConfig => slot.name == "settings.json",
        Scope::MenuVerb { path, .. } => {
            crate::menu::valid_verb_path(path) && slot.name == "LegacyDisable"
        }
        Scope::BlockedExtensions | Scope::MachineBlockedExtensions => {
            uuid::Uuid::parse_str(slot.name.trim_matches(['{', '}'])).is_ok()
        }
        // Existing journals must remain restorable after the brand rename.
        Scope::Startup => matches!(
            slot.name.as_str(),
            "WinGlow-Breeze" | "WindowsWeitiao-Breeze" | "WinGlow-TranslucentTB"
        ),
    };
    if !allowed {
        bail!("备份中包含不受本工具管理的注册表项：{}。", slot.name);
    }
    Ok(())
}

pub fn validate_entries(entries: &[Entry]) -> Result<()> {
    if entries.len() > 2000 {
        bail!("备份条目过多。");
    }
    let mut seen = std::collections::BTreeSet::new();
    for entry in entries {
        validate_slot(&entry.slot)?;
        if entry.slot.scope == Scope::TaskbarConfig {
            crate::taskbar::validate_config(entry.value.as_ref())?;
        }
        if entry.slot.scope == Scope::LiveUiFonts {
            crate::ui_fonts::validate(entry.value.as_ref())?;
        }
        if !seen.insert(&entry.slot) {
            bail!("备份存在重复条目。");
        }
        if let Some(value) = &entry.value
            && (value.bytes.len() > 65536 || !matches!(value.kind, 0 | 1 | 2 | 3 | 4 | 7))
        {
            bail!("备份值类型或大小不受支持。");
        }
    }
    Ok(())
}

pub struct WindowsRegistry;
impl ValueStore for WindowsRegistry {
    fn read(&self, slot: &Slot) -> Result<Option<StoredValue>> {
        validate_slot(slot)?;
        if slot.scope == Scope::TaskbarConfig {
            return crate::taskbar::read_config();
        }
        if slot.scope == Scope::LiveUiFonts {
            return Ok(Some(crate::ui_fonts::read()?));
        }
        let key = match slot
            .scope
            .root()
            .open_subkey_with_flags(slot.scope.path(), KEY_READ | KEY_WOW64_64KEY)
        {
            Ok(key) => key,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).context("读取字体注册表失败。"),
        };
        match key.get_raw_value(&slot.name) {
            Ok(value) => Ok(Some(StoredValue {
                kind: value.vtype as u32,
                bytes: value.bytes.to_vec(),
            })),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("读取字体值失败。"),
        }
    }
    fn write(&mut self, entry: &Entry) -> Result<()> {
        validate_entries(std::slice::from_ref(entry))?;
        if entry.slot.scope == Scope::TaskbarConfig {
            return crate::taskbar::write_config(entry.value.as_ref());
        }
        if entry.slot.scope == Scope::LiveUiFonts {
            return crate::ui_fonts::write(
                entry
                    .value
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("界面字体实时快照不能缺失。"))?,
            );
        }
        let root = entry.slot.scope.root();
        // Deleting a value from a missing key is already the desired state.
        let key = if entry.value.is_none() {
            match root.open_subkey_with_flags(
                entry.slot.scope.path(),
                KEY_READ | KEY_WRITE | KEY_WOW64_64KEY,
            ) {
                Ok(key) => key,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e).context("打开字体注册表失败。"),
            }
        } else {
            root.create_subkey_with_flags(
                entry.slot.scope.path(),
                KEY_READ | KEY_WRITE | KEY_WOW64_64KEY,
            )?
            .0
        };
        if let Some(value) = &entry.value {
            let vtype = match value.kind {
                0 => REG_NONE,
                1 => REG_SZ,
                2 => REG_EXPAND_SZ,
                3 => REG_BINARY,
                4 => REG_DWORD,
                7 => REG_MULTI_SZ,
                _ => bail!("不支持的注册表类型。"),
            };
            key.set_raw_value(
                &entry.slot.name,
                &RegValue {
                    bytes: value.bytes.clone().into(),
                    vtype,
                },
            )
            .context("写入字体设置失败。")?;
        } else {
            match key.delete_value(&entry.slot.name) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e).context("删除字体映射失败。"),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cannot_write_system_font_registration_or_emoji_mapping() {
        assert!(validate_slot(&slot(Scope::Fonts, "Segoe UI (TrueType)")).is_err());
        assert!(validate_slot(&slot(Scope::Substitutes, "Segoe UI Emoji")).is_err());
        assert!(validate_slot(&slot(Scope::Links, "HarmonyOS Sans SC")).is_ok());
    }
    #[test]
    fn unicode_registry_strings_round_trip() {
        assert_eq!(as_string(&string("苹方\\字体")), Some("苹方\\字体".into()));
    }
    #[test]
    fn cosmetic_switches_cannot_write_privilege_or_arbitrary_values() {
        assert!(validate_slot(&slot(Scope::Explorer, "EnableLUA")).is_err());
        assert!(validate_slot(&slot(Scope::IconOverrides, "29")).is_ok());
        assert!(validate_slot(&slot(Scope::IconOverrides, "77")).is_ok());
        assert!(validate_slot(&slot(Scope::Startup, "anything.exe")).is_err());
        assert!(validate_slot(&slot(Scope::SystemFonts, "arbitrary font (TrueType)")).is_err());
    }
}
