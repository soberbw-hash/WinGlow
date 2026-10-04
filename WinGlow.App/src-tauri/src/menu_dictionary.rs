//! Read-only GUID metadata. Network data can name/icon a registration, never create commands
//! or decide cleanup. Keep a bundled fallback and validate a download before replacement.
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::PathBuf,
    sync::{OnceLock, RwLock},
    time::Duration,
};

const BUNDLED: &str = include_str!("../../../third-party/ContextMenuManager/GuidInfosDic.ini");
const URL: &str = "https://raw.githubusercontent.com/BluePointLilac/ContextMenuManager/master/ContextMenuManager/Properties/Resources/Texts/GuidInfosDic.ini";
const LIMIT: usize = 256 * 1024;
#[derive(Clone, Default)]
pub struct Info {
    text: String,
    localized: String,
    resource: String,
    icon: String,
}
type Catalog = HashMap<String, Info>;
static CACHE: OnceLock<RwLock<Catalog>> = OnceLock::new();
fn normalized(guid: &str) -> Option<String> {
    uuid::Uuid::parse_str(guid.trim().trim_matches(['{', '}']))
        .ok()
        .map(|g| g.to_string())
}
fn parse(text: &str) -> Result<Catalog> {
    if text.len() > LIMIT || text.contains('\0') {
        bail!("菜单识别库格式无效。");
    }
    let mut result = Catalog::new();
    let mut section = None;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.starts_with(';') || line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = normalized(&line[1..line.len() - 1]);
            if let Some(id) = &section {
                result.entry(id.clone()).or_default();
            }
            continue;
        }
        if let Some(id) = &section
            && let Some((key, value)) = line.split_once('=')
        {
            let value = value.trim();
            if value.len() > 1024 || value.chars().any(|c| c.is_control()) {
                bail!("菜单识别库字段无效。");
            }
            let info = result.get_mut(id).unwrap();
            match key.trim().to_ascii_lowercase().as_str() {
                "text" => info.text = value.into(),
                "zh-cn-text" => info.localized = value.into(),
                "restext" => info.resource = value.into(),
                "icon" => info.icon = value.into(),
                _ => (), // XML commands, registry paths and scripts are never consumed.
            }
        }
    }
    result.retain(|_, v| !v.text.is_empty() || !v.localized.is_empty() || !v.resource.is_empty());
    if result.len() < 20 || result.len() > 4096 {
        bail!("菜单识别库内容不完整，保留原来的识别库。");
    }
    Ok(result)
}
fn root() -> Result<PathBuf> {
    Ok(crate::font_engine::data_root()?.join("MenuDictionary"))
}
fn catalog() -> &'static RwLock<Catalog> {
    CACHE.get_or_init(|| {
        let downloaded = root()
            .ok()
            .and_then(|p| read_cache(&p.join("GuidInfosDic.ini")).ok())
            .and_then(|s| parse(&s).ok());
        let mut bundled = parse(BUNDLED).expect("bundled dictionary was validated at build/test");
        if let Some(downloaded) = downloaded {
            bundled.extend(downloaded);
        }
        RwLock::new(bundled)
    })
}
fn read_cache(path: &std::path::Path) -> Result<String> {
    if fs::metadata(path)?.len() > LIMIT as u64 {
        bail!("菜单识别库过大。");
    }
    Ok(fs::read_to_string(path)?)
}
pub fn lookup(guid: &str) -> Option<Info> {
    catalog().read().ok()?.get(&normalized(guid)?).cloned()
}
fn resource_path(raw: &str, server: Option<&str>) -> Option<String> {
    let raw = raw.trim().trim_start_matches('@');
    let (file, index) = raw
        .rsplit_once(',')
        .filter(|(_, i)| i.trim().parse::<i32>().is_ok())
        .map(|(f, i)| (f.trim(), format!(",{}", i.trim())))
        .unwrap_or((raw, String::new()));
    let file = file.trim_matches('"');
    let path = if file == "*" {
        PathBuf::from(server?.trim_matches('"'))
    } else if let Some(relative) = file.strip_prefix(".\\") {
        if relative.contains(['\\', '/']) {
            return None;
        }
        PathBuf::from(server?.trim_matches('"'))
            .parent()?
            .join(relative)
    } else if !file.contains(['\\', '/', '%']) {
        // Relative system resources never use the current directory or PATH.
        crate::component_runtime::system_exe(file).ok()?
    } else {
        PathBuf::from(file)
    };
    let result = format!("{}{index}", path.display());
    crate::menu_icon::location(&result)?;
    Some(result)
}
impl Info {
    pub fn label(&self, server: Option<&str>) -> Option<String> {
        for value in [&self.resource, &self.localized, &self.text] {
            if value.is_empty() {
                continue;
            }
            if value.starts_with('@') {
                if let Some(path) = resource_path(value, server) {
                    let indirect = format!("@{path}");
                    let label = crate::menu::readable(indirect.clone());
                    if !label.is_empty() && label != indirect {
                        return Some(label);
                    }
                }
            } else {
                return Some(value.clone());
            }
        }
        None
    }
    pub fn icon(&self, server: Option<&str>) -> Option<String> {
        resource_path(&self.icon, server)
            .filter(|_| !self.icon.is_empty())
            .and_then(|p| crate::menu_icon::extract(&p))
    }
}
pub fn update_if_due() {
    let due = root()
        .ok()
        .and_then(|r| fs::metadata(r.join("checked")).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .is_none_or(|elapsed| elapsed >= Duration::from_secs(86400));
    if due {
        let _ = refresh();
    }
}
pub fn refresh() -> Result<crate::models::ActionResult> {
    let dir = root()?;
    fs::create_dir_all(&dir)?;
    // An attempted check is bounded to once daily even when the network is unavailable.
    fs::write(dir.join("checked"), b"checked")?;
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .get(URL)
        .send()?
        .error_for_status()?;
    let mut bytes = Vec::new();
    response.take(LIMIT as u64 + 1).read_to_end(&mut bytes)?;
    let text = std::str::from_utf8(&bytes)?;
    let downloaded = parse(text)?;
    let target = dir.join("GuidInfosDic.ini");
    if let Ok(old) = read_cache(&target) {
        if old.as_bytes() == bytes {
            return Ok(crate::models::ActionResult {
                message: "菜单识别库已是最新。".into(),
            });
        }
        // Save the previous metadata before replacement; no Windows settings are written.
        let history = dir.join("previous.ini");
        let temp = dir.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        crate::transaction::persist_new(&temp, old.as_bytes())?;
        crate::worker::replace_file(&temp, &history)?;
    }
    let temp = dir.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    crate::transaction::persist_new(&temp, &bytes)?;
    crate::worker::replace_file(&temp, &target)?;
    fs::write(
        dir.join("source.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"url":URL,"sha256":format!("{:x}",Sha256::digest(&bytes)),"entries":downloaded.len()}),
        )?,
    )?;
    let mut merged = parse(BUNDLED)?;
    merged.extend(downloaded);
    *catalog()
        .write()
        .map_err(|_| anyhow::anyhow!("菜单识别库正在加载。"))? = merged;
    Ok(crate::models::ActionResult {
        message: "菜单识别库已更新。".into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_dictionary_is_complete_and_names_defender() {
        let c = parse(BUNDLED).unwrap();
        assert!(c.len() > 100);
        assert!(c.contains_key("09a47860-11b0-4da5-afa5-26d86198a780"));
    }
    #[test]
    fn reject_partial_download_and_network_icon_paths() {
        assert!(parse("[bad]\nText=whatever").is_err());
        assert!(resource_path("\\\\server\\a.dll", None).is_none());
        assert!(resource_path("..\\a.dll", Some("C:\\App\\x.dll")).is_none());
        assert!(resource_path("https://host/icon.ico", None).is_none());
    }
}
