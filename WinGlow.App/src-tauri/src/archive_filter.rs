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
    let rules = serde_json::json!({"exact":exact,"patterns":patterns});
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
