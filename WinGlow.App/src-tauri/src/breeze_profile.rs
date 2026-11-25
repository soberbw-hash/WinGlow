//! Breeze settings selected by the user for WinGlow's one-click optimization.
use crate::registry::{Entry, Scope, StoredValue, slot};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::{fs, path::PathBuf};

const CONFIG_NAME: &str = "config.json";
const MAX_CONFIG_BYTES: usize = 65_536;

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
    let mut entries = crate::breeze_plugins::plan()?;
    let current = read()?;
    let mut config = match &current {
        Some(value) => serde_json::from_slice::<Value>(&value.bytes)
            .context("Breeze 配置无法读取，一键优化未修改它")?,
        None => Value::Object(Map::new()),
    };
    let root = config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Breeze 配置根节点不是对象，一键优化未修改它"))?;
    let before = serde_json::to_vec(root)?;
    apply_profile(root).context("应用 WinGlow 的 Breeze 默认设置失败")?;
    if serde_json::to_vec(root)? == before {
        return Ok(entries);
    }

    let bytes = serde_json::to_vec_pretty(&config)?;
    if bytes.len() > MAX_CONFIG_BYTES {
        bail!("Breeze 配置超过备份上限，一键优化未修改它。");
    }
    entries.push(Entry {
        slot: slot(Scope::BreezeConfig, CONFIG_NAME),
        value: Some(StoredValue { kind: 3, bytes }),
    });
    Ok(entries)
}

fn apply_profile(root: &mut Map<String, Value>) -> Result<()> {
    root.shift_remove("default_animation");
    root.insert("debug_console".into(), Value::Bool(false));
    root.insert("language".into(), Value::String("zh_CN".into()));
    root.insert("show_settings_button".into(), Value::Bool(true));
    root.insert("patch_explorerframe_dll".into(), Value::Bool(true));
    let mut order: Vec<Value> = crate::breeze_plugins::default_names()
        .map(|name| Value::String(name.into()))
        .collect();
    if let Some(previous) = root.get("plugin_load_order").and_then(Value::as_array) {
        for plugin in previous {
            if !order.contains(plugin) {
                order.push(plugin.clone());
            }
        }
    }
    root.insert("plugin_load_order".into(), Value::Array(order));
    let context_menu = root
        .entry("context_menu")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Breeze context_menu 配置不是对象"))?;
    context_menu.insert("vsync".into(), Value::Bool(true));
    context_menu.insert("ignore_owner_draw".into(), Value::Bool(true));
    context_menu.insert("reverse_if_open_to_up".into(), Value::Bool(false));
    context_menu.insert("hotkeys".into(), Value::Bool(true));
    let theme = context_menu.entry("theme").or_insert(Value::Null);
    if theme.is_null() {
        *theme = Value::Object(Map::new());
    }
    let theme = theme
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Breeze 菜单主题配置不是对象"))?;
    // Breeze 0.1.34 compares serialized animation groups, so keep upstream key order.
    let preset = json!({
        "radius": 6, "item_height": 24, "item_gap": 4, "item_radius": 8,
        "margin": 6, "padding": 6, "text_padding": 8, "icon_padding": 4,
        "right_icon_padding": 20, "multibutton_line_gap": -6, "acrylic": true,
        "animation": {
            "item": {
                "opacity": { "delay_scale": 0 },
                "width": { "easing": "mutation" },
                "x": { "easing": "mutation" },
                "blur": { "easing": "mutation" }
            },
            "submenu_bg": { "opacity": { "delay_scale": 0, "duration": 100 } },
            "main_bg": { "opacity": { "easing": "mutation" } }
        }
    });
    theme.extend(preset.as_object().unwrap().clone());
    Ok(())
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
            "plugin_load_order": ["New+", "链接详情显示"]
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
        let theme = &root["context_menu"]["theme"];
        assert_eq!(theme["item_height"], 24);
        assert_eq!(theme["item_gap"], 4);
        assert_eq!(theme["acrylic"], true);
        assert_eq!(theme["animation"]["submenu_bg"]["opacity"]["duration"], 100);
        assert_eq!(
            theme["animation"]["item"].to_string(),
            r#"{"opacity":{"delay_scale":0},"width":{"easing":"mutation"},"x":{"easing":"mutation"},"blur":{"easing":"mutation"}}"#
        );
        assert_eq!(root["context_menu"]["position"]["padding_vertical"], 20);
        assert_eq!(
            root["plugin_load_order"],
            json!([
                "New+",
                "右键菜单清理 - Context Menu Cleaner",
                "链接详情显示"
            ])
        );
    }

    #[test]
    fn malformed_or_wrong_shaped_config_is_rejected_without_replacement() {
        let mut root = Map::new();
        root.insert("context_menu".into(), Value::String("invalid".into()));
        assert!(apply_profile(&mut root).is_err());
    }
}
