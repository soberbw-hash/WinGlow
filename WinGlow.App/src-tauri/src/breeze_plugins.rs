use crate::registry::{Entry, Scope, StoredValue, slot};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{io::Read, path::PathBuf, time::Duration};

// Pinned, unmodified releases from the Breeze 0.1.x plugin market.
const SOURCE: &str = "https://raw.githubusercontent.com/breeze-shell/plugins/1d3bc081b014f026e1a2c01a82cd50f0aa0ebc52/plugins";
const MAX_BYTES: usize = 65_536;
const DEFAULTS: &[(&str, &str, &str)] = &[
    (
        "New+",
        "New%2B.js",
        "218eef60d3e1c3ea3e0b3a511283263dc9fcc7c55b589ba73c3e0e51c08966f5",
    ),
    (
        "右键菜单清理 - Context Menu Cleaner",
        "contextmenu-cleaner.js",
        "69126c80652a052ec2e0535102da6f3f0964f2d0fad0a9bd702f86d5c06e3607",
    ),
];

pub fn default_names() -> impl Iterator<Item = &'static str> {
    DEFAULTS.iter().map(|(name, _, _)| *name)
}

pub fn valid_name(name: &str) -> bool {
    default_names()
        .any(|listed| name == format!("{listed}.js") || name == format!("{listed}.js.disabled"))
}

fn path(name: &str) -> Result<PathBuf> {
    ensure!(valid_name(name), "不支持的 Breeze 插件文件。");
    Ok(PathBuf::from(std::env::var("USERPROFILE")?)
        .join(".breeze-shell/scripts")
        .join(name))
}

pub fn read(name: &str) -> Result<Option<StoredValue>> {
    crate::archive_filter::read_at(path(name)?)
}

pub fn write(name: &str, value: Option<&StoredValue>) -> Result<()> {
    crate::archive_filter::write_at(path(name)?, value)
}

pub fn plan() -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for &(name, upstream, digest) in DEFAULTS {
        let active = format!("{name}.js");
        if read(&active)?.is_some() {
            continue;
        }
        let disabled = format!("{active}.disabled");
        let previous = read(&disabled)?;
        let value = if let Some(value) = &previous {
            ensure!(
                value.bytes.len() <= MAX_BYTES,
                "Breeze 插件文件过大：{name}"
            );
            value.clone()
        } else {
            download(name, upstream, digest)?
        };
        entries.push(Entry {
            slot: slot(Scope::BreezePlugin, active),
            value: Some(value),
        });
        if previous.is_some() {
            entries.push(Entry {
                slot: slot(Scope::BreezePlugin, disabled),
                value: None,
            });
        }
    }
    Ok(entries)
}

fn download(name: &str, upstream: &str, digest: &str) -> Result<StoredValue> {
    let mut bytes = Vec::new();
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()?
        .get(format!("{SOURCE}/{upstream}"))
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .with_context(|| format!("无法下载 Breeze 默认插件 {name}，请检查网络后重试。"))?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_BYTES && format!("{:x}", Sha256::digest(&bytes)) == digest,
        "Breeze 默认插件 {name} 校验失败。"
    );
    Ok(StoredValue { kind: 3, bytes })
}
