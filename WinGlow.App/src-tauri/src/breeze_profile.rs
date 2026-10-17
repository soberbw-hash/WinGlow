//! Breeze settings selected by the user for WinGlow's one-click optimization.
use crate::registry::{Entry, Scope, StoredValue, slot};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::{fs, path::PathBuf};

const CONFIG_NAME: &str = "config.json";
const MAX_CONFIG_BYTES: usize = 65_536;
const PLUGINS: &[&str] = &[
    "New+",
    "Windows 11 Icon Pack (Ru - En)",
    "Windows 11 Icon Pack",
    "右键菜单清理 - Context Menu Cleaner",
    "常用功能小图标 - Copy paste etc. in one line",
    "链接详情显示",
];

fn path() -> Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("USERPROFILE")?).join(".breeze-shell/config.json"))
}

pub fn read() -> Result<Option<StoredValue>> {
    match fs::read(path()?) {
        Ok(bytes) => Ok(Some(StoredValue { kind: 3, bytes })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn write(value: Option<&StoredValue>) -> Result<()> {
    let path = path()?;
    if let Some(value) = value {
        if value.kind != 3 || value.bytes.len() > MAX_CONFIG_BYTES {
            bail!("Breeze 配置备份无效。");
        }
        // Validate before replacing the user's file. A transaction snapshot may
        // contain a config without our preset, but it must still be valid JSON.
        serde_json::from_slice::<Value>(&value.bytes).context("Breeze 配置不是有效 JSON")?;
        fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        crate::transaction::persist_new(&tmp, &value.bytes)?;
        crate::worker::replace_file(&tmp, &path)?;
    } else {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub fn plan() -> Result<Vec<Entry>> {
    let current = read()?;
    let mut config = match &current {
        Some(value) => serde_json::from_slice::<Value>(&value.bytes)
            .context("Breeze 配置无法读取，一键优化未修改它")?,
        None => Value::Object(Map::new()),
    };
    let root = config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Breeze 配置根节点不是对象，一键优化未修改它"))?;
    apply_profile(root).context("应用 WinGlow 的 Breeze 默认设置失败")?;

    let bytes = serde_json::to_vec_pretty(&config)?;
    if bytes.len() > MAX_CONFIG_BYTES {
        bail!("Breeze 配置超过备份上限，一键优化未修改它。");
    }
    let value = StoredValue { kind: 3, bytes };
    if current.as_ref() == Some(&value) {
        return Ok(Vec::new());
    }
    Ok(vec![Entry {
        slot: slot(Scope::BreezeConfig, CONFIG_NAME),
        value: Some(value),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_matches_the_selected_breeze_defaults_and_keeps_unrelated_settings() {
        let mut config = json!({
            "$schema": "schema-url",
            "default_animation": { "easing": "linear" },
            "debug_console": true,
            "font": { "main": ["Segoe UI"], "locale": "zh-CN" },
            "context_menu": {
                "theme": { "radius": 14 },
                "vsync": false,
                "ignore_owner_draw": false,
                "reverse_if_open_to_up": true,
                "hotkeys": false,
                "position": { "padding_vertical": 20 }
            },
            "plugin_load_order": []
        });
        let root = config.as_object_mut().unwrap();
        apply_profile(root).unwrap();

        assert_eq!(root["debug_console"], false);
        assert_eq!(root["language"], "zh_CN");
        assert_eq!(root["show_settings_button"], true);
        assert_eq!(root["patch_explorerframe_dll"], true);
        assert!(root.get("default_animation").is_none());
        assert_eq!(root["font"]["locale"], "zh-CN");
        assert_eq!(root["context_menu"]["vsync"], true);
        assert_eq!(root["context_menu"]["ignore_owner_draw"], true);
        assert_eq!(root["context_menu"]["reverse_if_open_to_up"], false);
        assert_eq!(root["context_menu"]["hotkeys"], true);
        assert!(root["context_menu"].get("theme").is_none());
        assert_eq!(root["context_menu"]["position"]["padding_vertical"], 20);
        assert_eq!(root["plugin_load_order"], json!(PLUGINS));
    }

    #[test]
    fn malformed_or_wrong_shaped_config_is_rejected_without_replacement() {
        let mut root = Map::new();
        root.insert("context_menu".into(), Value::String("invalid".into()));
        assert!(apply_profile(&mut root).is_err());
    }
}

fn apply_profile(root: &mut Map<String, Value>) -> Result<()> {
    root.remove("default_animation");
    root.insert("debug_console".into(), Value::Bool(false));
    root.insert("language".into(), Value::String("zh_CN".into()));
    root.insert("show_settings_button".into(), Value::Bool(true));
    root.insert("patch_explorerframe_dll".into(), Value::Bool(true));
    let context_menu = root
        .entry("context_menu")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Breeze context_menu 配置不是对象"))?;
    context_menu.remove("theme");
    context_menu.insert("vsync".into(), Value::Bool(true));
    context_menu.insert("ignore_owner_draw".into(), Value::Bool(true));
    context_menu.insert("reverse_if_open_to_up".into(), Value::Bool(false));
    context_menu.insert("hotkeys".into(), Value::Bool(true));
    root.insert("plugin_load_order".into(), json!(PLUGINS));
    Ok(())
}
