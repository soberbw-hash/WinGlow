//! Selectively decode v2 backups without running reg.exe or importing unrelated settings.
use crate::{
    preset_data::MANAGED_ALIASES,
    registry::{self, Entry, Scope, StoredValue},
};
use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, fs, path::Path};

fn quoted(text: &str) -> Result<(String, &str)> {
    let Some(rest) = text.strip_prefix('"') else {
        bail!("旧版备份格式错误。")
    };
    let mut result = String::new();
    let mut escaped = false;
    for (index, c) in rest.char_indices() {
        if escaped {
            if c != '"' && c != '\\' {
                bail!("旧版备份包含未知转义。")
            }
            result.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Ok((result, &rest[index + 1..]));
        } else {
            result.push(c);
        }
    }
    bail!("旧版备份字符串未闭合。")
}
fn value(text: &str) -> Result<Option<StoredValue>> {
    if text == "-" {
        return Ok(None);
    }
    if text.starts_with('"') {
        let (s, rest) = quoted(text)?;
        if !rest.trim().is_empty() {
            bail!("旧版备份值格式错误。")
        }
        return Ok(Some(registry::string(&s)));
    }
    if let Some(hex) = text.strip_prefix("dword:") {
        return Ok(Some(StoredValue {
            kind: 4,
            bytes: u32::from_str_radix(hex, 16)?.to_le_bytes().to_vec(),
        }));
    }
    let (kind, hex) = if let Some(hex) = text.strip_prefix("hex(7):") {
        (7, hex)
    } else if let Some(hex) = text.strip_prefix("hex(2):") {
        (2, hex)
    } else {
        bail!("旧版备份包含不支持的字体值类型。")
    };
    let bytes = hex
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| u8::from_str_radix(s.trim(), 16))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(Some(StoredValue { kind, bytes }))
}
fn decode(bytes: &[u8]) -> Result<String> {
    if bytes.len() > 2_000_000 {
        bail!("旧版备份过大。")
    }
    if bytes.starts_with(&[0xff, 0xfe]) {
        if !bytes.len().is_multiple_of(2) {
            bail!("旧版备份 UTF-16 内容不完整。")
        }
        Ok(String::from_utf16(
            &bytes[2..]
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )?)
    } else {
        Ok(String::from_utf8(bytes.to_vec())?
            .trim_start_matches('\u{feff}')
            .into())
    }
}
pub fn parse(bytes: &[u8], scope: Scope) -> Result<Vec<Entry>> {
    let text = decode(bytes)?;
    if !text.starts_with("Windows Registry Editor Version 5.00") {
        bail!("旧版备份缺少有效的注册表文件头。")
    }
    let root = if matches!(scope, Scope::Desktop | Scope::Avalon(_)) {
        "HKEY_CURRENT_USER"
    } else {
        "HKEY_LOCAL_MACHINE"
    };
    let target = format!("[{root}\\{}]", scope.path());
    let mut in_target = false;
    let mut found = false;
    let mut pending = String::new();
    let mut parsed = BTreeMap::new();
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            in_target = line.eq_ignore_ascii_case(&target);
            found |= in_target;
            continue;
        }
        if !in_target {
            continue;
        }
        pending.push_str(line.trim_end_matches('\\'));
        if line.ends_with('\\') {
            continue;
        }
        let complete = std::mem::take(&mut pending);
        if !complete.starts_with('"') {
            continue;
        }
        let (name, rest) = quoted(&complete)?;
        let s = registry::slot(scope.clone(), name.clone());
        if registry::validate_slot(&s).is_err() {
            continue;
        }
        let raw = rest
            .strip_prefix('=')
            .ok_or_else(|| anyhow::anyhow!("旧版备份赋值格式错误。"))?;
        if parsed.insert(name, value(raw)?).is_some() {
            bail!("旧版备份包含重复值。")
        }
    }
    if !pending.is_empty() {
        bail!("旧版备份值不完整。")
    }
    if !found {
        bail!("旧版备份缺少目标注册表节：{target}。")
    }
    let names: Vec<String> = match &scope {
        Scope::Substitutes | Scope::Links => {
            MANAGED_ALIASES.iter().map(|s| s.to_string()).collect()
        }
        Scope::Desktop => [
            "FontSmoothing",
            "FontSmoothingType",
            "FontSmoothingGamma",
            "FontSmoothingOrientation",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        Scope::Avalon(_) => [
            "PixelStructure",
            "GammaLevel",
            "ClearTypeLevel",
            "TextContrastLevel",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        _ => bail!("不迁移此类旧版记录。"),
    };
    let entries: Vec<_> = names
        .into_iter()
        .map(|name| Entry {
            value: parsed.remove(&name).flatten(),
            slot: registry::slot(scope.clone(), name),
        })
        .collect();
    registry::validate_entries(&entries)?;
    Ok(entries)
}

pub fn read_backup(dir: &Path) -> Result<Vec<Entry>> {
    let mut result = parse(
        &fs::read(dir.join("FontSubstitutes.reg"))?,
        Scope::Substitutes,
    )?;
    for (file, scope) in [
        ("FontLink.reg", Scope::Links),
        ("Desktop.reg", Scope::Desktop),
    ] {
        let path = dir.join(file);
        if path.is_file() {
            result.extend(parse(
                &fs::read(path).with_context(|| format!("读取旧版备份 {file} 失败。"))?,
                scope,
            )?);
        }
    }
    let path = dir.join("Avalon.Graphics.reg");
    if path.is_file() {
        let bytes = fs::read(&path)?;
        let text = decode(&bytes)?;
        let prefix = "[HKEY_CURRENT_USER\\Software\\Microsoft\\Avalon.Graphics\\";
        for display in text.lines().filter_map(|l| {
            l.trim()
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(']'))
        }) {
            let scope = Scope::Avalon(display.into());
            registry::validate_slot(&registry::slot(scope.clone(), "GammaLevel"))?;
            result.extend(parse(&bytes, scope)?);
        }
    }
    registry::validate_entries(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restore_absent_values_and_ignore_other_registry_sections() {
        let text = "Windows Registry Editor Version 5.00\n[HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\FontSubstitutes]\n\"Segoe UI\"=\"before\"\n[HKEY_LOCAL_MACHINE\\Other]\n\"Microsoft YaHei\"=\"unrelated\"\n";
        let entries = parse(text.as_bytes(), Scope::Substitutes).unwrap();
        assert_eq!(entries[0].value, Some(registry::string("before")));
        assert_eq!(
            entries
                .iter()
                .find(|e| e.slot.name == "Microsoft YaHei")
                .unwrap()
                .value,
            None
        );
    }
    #[test]
    fn multiline_link_bytes_and_escaped_strings_are_preserved() {
        let text = "Windows Registry Editor Version 5.00\n[HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\FontLink\\SystemLink]\n\"Segoe UI\"=hex(7):61,00,\\\n00,00,00,00\n";
        assert_eq!(
            parse(text.as_bytes(), Scope::Links).unwrap()[0].value,
            Some(StoredValue {
                kind: 7,
                bytes: vec![97, 0, 0, 0, 0, 0]
            })
        );
        assert_eq!(
            quoted("\"C:\\\\Fonts\\\\字体\"=").unwrap().0,
            "C:\\Fonts\\字体"
        );
    }
    #[test]
    fn reject_empty_partial_or_missing_section_backups() {
        assert!(parse(b"", Scope::Substitutes).is_err());
        assert!(parse(b"Windows Registry Editor Version 5.00", Scope::Substitutes).is_err());
        assert!(value("hex(7):xy").is_err());
    }
}
