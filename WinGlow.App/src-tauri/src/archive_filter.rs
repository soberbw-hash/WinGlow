use crate::registry::{Entry, Scope, StoredValue};
use anyhow::{Result, bail};
use std::{fs, path::PathBuf};
const SCRIPT: &[u8] = include_bytes!("archive_filter.js");
fn path() -> Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("USERPROFILE")?)
        .join(".breeze-shell/scripts/WinGlow-ArchiveFilter.js"))
}
pub fn read() -> Result<Option<StoredValue>> {
    match fs::read(path()?) {
        Ok(bytes) => Ok(Some(StoredValue { kind: 3, bytes })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn write(value: Option<&StoredValue>) -> Result<()> {
    let path = path()?;
    if let Some(value) = value {
        if value.kind != 3 || value.bytes.len() > 65536 {
            bail!("菜单过滤快照无效。");
        }
        fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        crate::transaction::persist_new(&tmp, &value.bytes)?;
        crate::worker::replace_file(&tmp, &path)?;
    } else {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub fn plan() -> Result<Vec<Entry>> {
    plan_with(&[])
}
pub fn plan_with(changes: &[Entry]) -> Result<Vec<Entry>> {
    let items = crate::menu::scan_impl(false)?;
    let rules = display_rules(items, changes);
    let script = String::from_utf8(SCRIPT.to_vec())?.replace(
        "/*WinGlowRules*/ { exact: [], patterns: [] }",
        &serde_json::to_string(&rules)?,
    );
    let value = StoredValue {
        kind: 3,
        bytes: script.into_bytes(),
    };
    if value.bytes.len() > 65536 {
        bail!("菜单过滤规则超过备份上限，未修改。");
    }
    if read()?.as_ref() == Some(&value) {
        return Ok(vec![]);
    }
    Ok(vec![Entry {
        slot: crate::registry::slot(Scope::ArchiveFilter, "WinGlow-ArchiveFilter.js"),
        value: Some(value),
    }])
}
fn display_rules(
    items: Vec<(crate::models::MenuItem, crate::registry::Slot)>,
    changes: &[Entry],
) -> serde_json::Value {
    let mut disabled = Vec::new();
    let mut enabled = std::collections::BTreeSet::new();
    for (item, slot) in items {
        let visible = changes
            .iter()
            .rev()
            .find(|entry| entry.slot == slot)
            .map_or(item.enabled, |entry| {
                if let Scope::MenuHandler { .. } = slot.scope {
                    entry
                        .value
                        .as_ref()
                        .and_then(crate::registry::as_string)
                        .is_some_and(|value| !value.starts_with('-'))
                } else {
                    entry.value.is_none()
                }
            });
        if visible {
            enabled.insert(item.label.clone());
            enabled.insert(item.raw_label.clone());
        } else {
            disabled.push(item);
        }
    }
    let mut exact = std::collections::BTreeSet::new();
    for item in &disabled {
        for label in [&item.label, &item.raw_label] {
            if !enabled.contains(label) && !label.is_empty() {
                exact.insert(label.clone());
            }
        }
    }
    let off = |terms: &[&str]| {
        disabled.iter().any(|item| {
            terms.iter().any(|term| {
                format!("{} {}", item.label, item.raw_label)
                    .to_lowercase()
                    .contains(term)
            })
        }) && !enabled
            .iter()
            .any(|label| terms.iter().any(|term| label.to_lowercase().contains(term)))
    };
    let mut patterns = Vec::new();
    for (terms, pattern) in [
        (&["workbuddy", "沃克巴迪"][..], "workbuddy|沃克巴迪"),
        (&["quark", "夸克"][..], "夸克|quark"),
        (
            &["百度网盘", "baidu", "yunshellext"][..],
            "百度网盘|baidunetdisk",
        ),
        (&["defender", "epp"][..], "defender"),
        (
            &["previous versions", "以前的版本"][..],
            "^(?:还原)?以前的版本$|previousversions",
        ),
        (
            &["encryption context", "加密 / 解密"][..],
            "^(?:加密|解密)$",
        ),
        (&["slideshow", "幻灯片"][..], "幻灯片|slideshow"),
        (
            &["微信输入法", "wetype"][..],
            "微信输入法.*(?:传送|传输)|wetype",
        ),
    ] {
        if off(terms) {
            patterns.push(pattern);
        }
    }
    // Both cloud providers use these unbranded labels. Keep them if either is ON.
    if off(&["quark", "夸克"]) && off(&["百度网盘", "baidu", "yunshellext"]) {
        patterns.push("^(?:自动备份(?:该|此)?文件夹|用手机打开|发送到手机|同步至其[它他]设备)$");
    }
    serde_json::json!({"exact":exact,"patterns":patterns,"enabled":enabled})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(
        label: &str,
        enabled: bool,
        scope: Scope,
    ) -> (crate::models::MenuItem, crate::registry::Slot) {
        (
            crate::models::MenuItem {
                id: label.into(),
                label: label.into(),
                raw_label: label.into(),
                enabled,
                group: "文件夹".into(),
                kind: "当前用户".into(),
                targets: vec!["文件夹".into()],
                auto_hide: false,
                menu_level: "direct".into(),
                children: vec![],
                sub_items: vec![],
                visibility_note: None,
                icon_data_url: None,
                icon_source: None,
            },
            crate::registry::slot(scope, "LegacyDisable"),
        )
    }
    fn verb() -> Scope {
        Scope::MenuVerb {
            machine: false,
            path: "Directory\\shell\\WorkBuddy".into(),
        }
    }
    fn handler() -> Scope {
        Scope::MenuHandler {
            machine: false,
            path: "Directory\\shellex\\ContextMenuHandlers\\Quark".into(),
            guid: "{D7D43EA6-BCE2-489C-9A70-9027A42E1EFD}".into(),
        }
    }
    #[test]
    fn rules_use_post_transaction_state_for_manual_switches() {
        let (before, slot) = item("用 WorkBuddy 打开", true, verb());
        let disabled = display_rules(
            vec![(before, slot.clone())],
            &[crate::menu::visibility_entry(slot.clone(), false)],
        );
        assert!(
            disabled["exact"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "用 WorkBuddy 打开")
        );
        let (before, _) = item("用 WorkBuddy 打开", false, verb());
        let enabled = display_rules(
            vec![(before, slot.clone())],
            &[crate::menu::visibility_entry(slot, true)],
        );
        assert!(enabled["patterns"].as_array().unwrap().is_empty());
        assert!(enabled["exact"].as_array().unwrap().is_empty());
        assert_eq!(enabled["enabled"][0], "用 WorkBuddy 打开");
    }
    #[test]
    fn handler_projection_and_cloud_aliases_require_all_related_sources_off() {
        let (quark, mut slot) = item("夸克网盘", true, handler());
        slot.name.clear();
        let (baidu, baidu_slot) = item("百度网盘", false, Scope::BlockedExtensions);
        let off = display_rules(
            vec![
                (quark.clone(), slot.clone()),
                (baidu.clone(), baidu_slot.clone()),
            ],
            &[crate::menu::visibility_entry(slot.clone(), false)],
        );
        assert!(
            off["patterns"]
                .as_array()
                .unwrap()
                .iter()
                .any(|pattern| pattern.as_str().unwrap().contains("发送到手机"))
        );
        let on = display_rules(vec![(quark, slot), (baidu, baidu_slot)], &[]);
        assert!(
            !on["patterns"]
                .as_array()
                .unwrap()
                .iter()
                .any(|pattern| pattern.as_str().unwrap().contains("发送到手机"))
        );
    }
    #[test]
    fn enabled_titles_are_retained_for_display_collision_protection() {
        let rules = display_rules(
            vec![
                item("Open Project", false, verb()),
                item("OPEN PROJECT(&P)", true, verb()),
            ],
            &[],
        );
        assert_eq!(rules["enabled"][0], "OPEN PROJECT(&P)");
        assert_eq!(rules["exact"][0], "Open Project");
    }
}
