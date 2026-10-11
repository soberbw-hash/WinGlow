//! Fixed, journalled settings for the owned taskbar and optional classic layout.
use crate::{
    models::ActionResult,
    registry::{self, Entry, Scope, WindowsRegistry},
    shell_engine, taskbar,
    transaction::ValueStore,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskbarLook {
    pub accent: String,
    pub color: String,
    pub opacity: u8,
    pub show_line: bool,
}
impl Default for TaskbarLook {
    fn default() -> Self {
        Self {
            accent: "blur".into(),
            color: "#000000".into(),
            opacity: 0,
            show_line: true,
        }
    }
}
impl TaskbarLook {
    fn validate(&self) -> Result<()> {
        if !matches!(
            self.accent.as_str(),
            "normal" | "opaque" | "clear" | "blur" | "acrylic"
        ) || self.opacity > 100
            || self.color.len() != 7
            || !self.color.starts_with('#')
            || !self.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            bail!("任务栏参数无效。");
        }
        Ok(())
    }
    fn json(&self, enabled: bool) -> serde_json::Value {
        serde_json::json!({"accent":self.accent,"color":format!("{}{:02X}",self.color, (u32::from(self.opacity)*255+50)/100),"show_line":self.show_line,"enabled":enabled})
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskbarSettings {
    pub desktop: TaskbarLook,
    pub maximized: Option<TaskbarLook>,
    pub start_open: Option<TaskbarLook>,
}
pub fn taskbar_settings() -> Result<TaskbarSettings> {
    let Some(value) = taskbar::read_config()? else {
        return Ok(TaskbarSettings::default());
    };
    let json: serde_json::Value = serde_json::from_slice(&value.bytes)?;
    fn look(value: &serde_json::Value) -> TaskbarLook {
        let color = value["color"].as_str().unwrap_or("#00000000");
        let (rgb, alpha) = if color.len() == 9
            && color.starts_with('#')
            && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            (
                color[..7].to_string(),
                u8::from_str_radix(&color[7..], 16).unwrap_or(0),
            )
        } else {
            ("#000000".into(), 0)
        };
        TaskbarLook {
            accent: value["accent"]
                .as_str()
                .filter(|s| matches!(*s, "normal" | "opaque" | "clear" | "blur" | "acrylic"))
                .unwrap_or("blur")
                .into(),
            color: rgb,
            opacity: ((u32::from(alpha) * 100 + 127) / 255) as u8,
            show_line: value["show_line"].as_bool().unwrap_or(true),
        }
    }
    Ok(TaskbarSettings {
        desktop: look(&json["desktop_appearance"]),
        maximized: json["maximized_window_appearance"]["enabled"]
            .as_bool()
            .unwrap_or(false)
            .then(|| look(&json["maximized_window_appearance"])),
        start_open: json["start_opened_appearance"]["enabled"]
            .as_bool()
            .unwrap_or(false)
            .then(|| look(&json["start_opened_appearance"])),
    })
}
fn taskbar_bytes(
    settings: &TaskbarSettings,
    previous: Option<&registry::StoredValue>,
) -> Result<Vec<u8>> {
    settings.desktop.validate()?;
    for look in [&settings.maximized, &settings.start_open]
        .into_iter()
        .flatten()
    {
        look.validate()?;
    }
    let mut json = match previous {
        Some(v) => serde_json::from_slice::<serde_json::Value>(&v.bytes)?,
        None => serde_json::json!({}),
    };
    let obj = json
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("任务栏配置无效，未覆盖。"))?;
    for (name, patch) in [
        ("desktop_appearance", settings.desktop.json(true)),
        (
            "maximized_window_appearance",
            settings
                .maximized
                .as_ref()
                .map_or(serde_json::json!({"enabled":false}), |v| v.json(true)),
        ),
        (
            "start_opened_appearance",
            settings
                .start_open
                .as_ref()
                .map_or(serde_json::json!({"enabled":false}), |v| v.json(true)),
        ),
    ] {
        let value = obj.entry(name).or_insert_with(|| serde_json::json!({}));
        let original = value
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("任务栏状态配置无效，未覆盖。"))?;
        for (key, value) in patch.as_object().unwrap() {
            if name != "desktop_appearance" || key != "enabled" {
                original.insert(key.clone(), value.clone());
            }
        }
    }
    obj.insert("disable_saving".into(), serde_json::json!(true));
    Ok(serde_json::to_vec_pretty(&json)?)
}
pub fn apply_taskbar(settings: TaskbarSettings) -> Result<ActionResult> {
    if classic_settings()?.taskbar != 0 {
        bail!("经典任务栏正在启用，请先恢复系统任务栏，再应用透明效果。");
    }
    let bytes = taskbar_bytes(&settings, taskbar::read_config()?.as_ref())?;
    taskbar::prepare()?;
    taskbar::pause_owned()?;
    let mut entries = taskbar::plan()?;
    entries[0].value = Some(registry::StoredValue { kind: 3, bytes });
    let result = shell_engine::commit("visual:taskbar-settings", entries, taskbar::sync);
    if result.is_err() {
        let _ = taskbar::sync();
    }
    result.map(|_| ActionResult {
        message: "任务栏设置已应用，无需重启资源管理器。".into(),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClassicSettings {
    pub taskbar: u32,
    pub alignment: u32,
    pub combine: u32,
    pub small_icons: bool,
    pub start_menu: u32,
}
pub fn classic_settings() -> Result<ClassicSettings> {
    fn number(scope: Scope, name: &str, default: u32) -> Result<u32> {
        Ok(WindowsRegistry
            .read(&registry::slot(scope, name))?
            .filter(|v| v.kind == 4 && v.bytes.len() == 4)
            .map(|v| u32::from_le_bytes(v.bytes.try_into().unwrap()))
            .unwrap_or(default))
    }
    Ok(ClassicSettings {
        taskbar: number(Scope::ClassicConfig, "OldTaskbar", 0)?,
        alignment: number(Scope::Explorer, "TaskbarAl", 1)?,
        combine: number(Scope::Explorer, "TaskbarGlomLevel", 0)?,
        small_icons: number(Scope::Explorer, "TaskbarSmallIcons", 0)? != 0,
        start_menu: number(Scope::Explorer, "Start_ShowClassicMode", 0)?,
    })
}
fn classic_plan(settings: &ClassicSettings) -> Result<Vec<Entry>> {
    if !matches!(settings.taskbar, 0 | 2)
        || settings.alignment > 1
        || settings.combine > 2
        || settings.start_menu > 1
    {
        bail!("经典布局参数无效。");
    }
    let mut entries = vec![];
    for (scope, name, value) in [
        (Scope::ClassicConfig, "OldTaskbar", settings.taskbar),
        (Scope::ClassicConfig, "OldTaskbarAl", settings.alignment),
        (Scope::ClassicConfig, "MMOldTaskbarAl", settings.alignment),
        (Scope::Explorer, "TaskbarAl", settings.alignment),
        (Scope::Explorer, "TaskbarGlomLevel", settings.combine),
        (Scope::Explorer, "MMTaskbarGlomLevel", settings.combine),
        (
            Scope::Explorer,
            "TaskbarSmallIcons",
            u32::from(settings.small_icons),
        ),
        (
            Scope::Explorer,
            "Start_ShowClassicMode",
            settings.start_menu,
        ),
    ] {
        entries.push(Entry {
            slot: registry::slot(scope, name),
            value: Some(shell_engine::dword(value)),
        });
    }
    Ok(entries)
}
pub fn apply_classic(settings: ClassicSettings) -> Result<ActionResult> {
    crate::optional_tools::ensure_classic_settings_supported()?;
    if settings.start_menu == 1 && crate::visual::start_enabled()? {
        bail!("请先关闭开始菜单美化，再使用经典开始菜单，避免两个组件同时调整开始菜单。");
    }
    let mut entries = classic_plan(&settings)?;
    entries.push(Entry {
        slot: taskbar::startup_slot(),
        value: if settings.taskbar != 0 {
            None
        } else {
            WindowsRegistry.read(&taskbar::startup_slot())?
        },
    });
    let result = shell_engine::commit("visual:classic-settings", entries, || {
        if settings.taskbar != 0 {
            taskbar::pause_owned()
        } else {
            Ok(())
        }
    });
    if result.is_err() {
        let _ = taskbar::sync();
    }
    result.map(|_| ActionResult {
        message: "经典布局设置已保存。注销后生效，避免连续重启资源管理器。".into(),
    })
}
pub fn restore_classic() -> Result<ActionResult> {
    crate::optional_tools::ensure_classic_settings_supported()?;
    let mut names = classic_plan(&ClassicSettings {
        taskbar: 0,
        alignment: 1,
        combine: 0,
        small_icons: false,
        start_menu: 0,
    })?
    .into_iter()
    .map(|e| e.slot)
    .collect::<Vec<_>>();
    names.push(taskbar::startup_slot());
    // This feature's baseline is distinct from the earlier one-click baseline.
    // Restoring layout should retain the taskbar that was enabled before it.
    let mut original = std::collections::BTreeMap::new();
    for dir in crate::transaction::backup_directories(&crate::font_engine::backup_root()?)?
        .iter()
        .rev()
    {
        if !dir.join("committed").is_file() {
            continue;
        }
        let journal = crate::transaction::read_journal(dir)?;
        if journal.operation != "visual:classic-settings" {
            continue;
        }
        for entry in journal.before {
            if names.contains(&entry.slot) {
                original.entry(entry.slot.clone()).or_insert(entry);
            }
        }
    }
    if original.is_empty() {
        return Ok(ActionResult {
            message: "没有需要恢复的经典布局设置。".into(),
        });
    }
    let entries = names
        .into_iter()
        .map(|slot| {
            Ok(original.remove(&slot).unwrap_or(Entry {
                value: WindowsRegistry.read(&slot)?,
                slot,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    shell_engine::commit("visual:classic-restore", entries, || Ok(())).map(|_| ActionResult {
        message: "已恢复经典布局修改前的设置，注销后生效。".into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn taskbar_changes_preserve_other_rules_and_validate_colors() {
        let previous = registry::StoredValue {
            kind: 3,
            bytes:
                br#"{"desktop_appearance":{"blur_radius":12},"maximized_window_appearance":{"rules":{"process_name":{"code.exe":{"accent":"normal"}}}},"search_opened_appearance":{"enabled":true,"accent":"normal"},"unknown":42}"#
                    .to_vec(),
        };
        let mut s = TaskbarSettings::default();
        s.desktop.opacity = 50;
        s.maximized = Some(TaskbarLook {
            accent: "opaque".into(),
            ..Default::default()
        });
        let v: serde_json::Value =
            serde_json::from_slice(&taskbar_bytes(&s, Some(&previous)).unwrap()).unwrap();
        assert_eq!(v["search_opened_appearance"]["enabled"], true);
        assert_eq!(v["unknown"], 42);
        assert_eq!(v["desktop_appearance"]["blur_radius"], 12);
        assert_eq!(
            v["maximized_window_appearance"]["rules"]["process_name"]["code.exe"]["accent"],
            "normal"
        );
        assert_eq!(v["desktop_appearance"]["color"], "#00000080");
        s.desktop.color = "#x00000".into();
        assert!(taskbar_bytes(&s, None).is_err());
    }
    #[test]
    fn classic_plan_rejects_arbitrary_modes_and_keeps_fixed_slots() {
        let mut s = ClassicSettings {
            taskbar: 2,
            alignment: 0,
            combine: 2,
            small_icons: true,
            start_menu: 0,
        };
        for e in classic_plan(&s).unwrap() {
            registry::validate_slot(&e.slot).unwrap();
        }
        s.taskbar = 999;
        assert!(classic_plan(&s).is_err());
    }
}
