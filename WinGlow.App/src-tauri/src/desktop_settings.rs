//! Journalled settings for the owned taskbar.
use crate::{
    models::ActionResult,
    registry::{self, Scope, WindowsRegistry},
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
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskbarSettings {
    pub desktop: TaskbarLook,
    pub maximized: Option<TaskbarLook>,
    pub start_open: Option<TaskbarLook>,
}
impl Default for TaskbarSettings {
    fn default() -> Self {
        Self {
            desktop: TaskbarLook::default(),
            maximized: Some(TaskbarLook::default()),
            start_open: Some(TaskbarLook::default()),
        }
    }
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
        None => serde_json::from_slice(&taskbar::config())?,
    };
    let obj = json
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("任务栏配置无效，未覆盖。"))?;
    for (name, patch) in [
        ("desktop_appearance", settings.desktop.json(true)),
        ("visible_window_appearance", settings.desktop.json(true)),
        ("search_opened_appearance", settings.desktop.json(true)),
        ("task_view_opened_appearance", settings.desktop.json(true)),
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
    if legacy_taskbar_active()? {
        bail!("第三方任务栏替代组件正在启用，请先恢复系统任务栏。");
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

pub fn legacy_taskbar_active() -> Result<bool> {
    Ok(WindowsRegistry
        .read(&registry::slot(Scope::ClassicConfig, "OldTaskbar"))?
        .is_some_and(|v| v.kind == 4 && v.bytes.as_slice() != 0u32.to_le_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_taskbar_application_keeps_all_optimization_defaults() {
        let settings = TaskbarSettings::default();
        let actual: serde_json::Value =
            serde_json::from_slice(&taskbar_bytes(&settings, None).unwrap()).unwrap();
        let expected: serde_json::Value = serde_json::from_slice(&taskbar::config()).unwrap();
        assert_eq!(actual, expected);
    }
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
        assert_eq!(v["search_opened_appearance"]["accent"], "blur");
        assert_eq!(v["visible_window_appearance"]["color"], "#00000080");
        assert_eq!(v["task_view_opened_appearance"]["color"], "#00000080");
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
}
