//! Owned, isolated Start Menu Styler. Only fixed files enter the recovery journal.
use crate::{
    component_runtime as runtime, font_engine,
    registry::{self, Entry, Scope, Slot, StoredValue, WindowsRegistry},
    shell_engine,
    transaction::{self, ValueStore},
};
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read},
    path::PathBuf,
    time::{Duration, Instant},
};

const ARCHIVE: &[u8] =
    include_bytes!("../../../third-party/Windhawk/Windhawk-1.7.3-start-menu-1.7.zip");
const HASH: &str = "5ee9b3f448c6a79a1c9b0e8de8cc2381fdcbb389c3642ae33158c84d68db32fe";
pub const CONFIG_NAMES: &[&str] = &["start-menu.ini", "windhawk-settings.ini", "material.json"];
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Appearance {
    pub material: u32,
    pub tint: u8,
    pub radius: u8,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            material: 3,
            tint: 78,
            radius: 16,
        }
    }
}
impl Appearance {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.material, 2 | 3) || !(20..=95).contains(&self.tint) || self.radius > 24 {
            bail!("美化参数无效，请重新选择。");
        }
        Ok(())
    }
}
pub fn appearance() -> Result<Appearance> {
    let mut settings = Appearance {
        material: crate::window_material::configured_material()?,
        ..Appearance::default()
    };
    if let Some(value) = read("start-menu.ini")? {
        let text = String::from_utf8_lossy(&value.bytes);
        if let Some(tint) = text
            .split("TintOpacity=\"")
            .nth(1)
            .and_then(|tail| tail.split('"').next())
            .and_then(|number| number.parse::<f64>().ok())
            .filter(|number| number.is_finite())
        {
            settings.tint = (tint * 100.0).round().clamp(20.0, 95.0) as u8;
        }
        if let Some(radius) = text
            .split("CornerRadius=")
            .nth(1)
            .and_then(|tail| tail.lines().next())
            .and_then(|number| number.trim().parse::<u8>().ok())
        {
            settings.radius = radius.min(24);
        }
    }
    Ok(settings)
}
pub fn apply_settings(id: &str, settings: Appearance) -> Result<crate::models::ActionResult> {
    settings.validate()?;
    if id == "window-material" {
        return crate::window_material::set_style(settings.material);
    }
    if id != "start-menu" {
        bail!("未知美化设置。");
    }
    prepare_start()?;
    let mut entries = start_plan()?;
    entries[0] = bytes_entry("start-menu.ini", start_preset_with(&settings));
    let result = shell_engine::commit("visual:start-menu-settings", entries, || {
        stop_start()?;
        sync_start()
    });
    if result.is_err() {
        let _ = sync_start();
    }
    result
}
pub fn build() -> u32 {
    winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
        .ok()
        .and_then(|k| k.get_value::<String, _>("CurrentBuildNumber").ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}
pub fn start_supported() -> bool {
    cfg!(target_arch = "x86_64") && build() >= 22000
}
pub fn start_available() -> bool {
    start_supported() && exe().is_ok_and(|exe| runtime::preflight(&exe).is_ok())
}
pub fn root() -> Result<PathBuf> {
    Ok(font_engine::data_root()?.join("Windhawk").join("1.7.3"))
}
fn exe() -> Result<PathBuf> {
    Ok(root()?.join("windhawk.exe"))
}
pub fn config_slot(name: &str) -> Slot {
    registry::slot(Scope::VisualConfig, name)
}
fn startup_slot() -> Slot {
    registry::slot(Scope::Startup, "WinGlow-StartMenu")
}
fn path(name: &str) -> Result<PathBuf> {
    Ok(match name {
        "start-menu.ini" => root()?.join("AppData/Engine/Mods/windows-11-start-menu-styler.ini"),
        "windhawk-settings.ini" => root()?.join("AppData/settings.ini"),
        "material.json" => font_engine::data_root()?.join("window-material.json"),
        _ => bail!("未知视觉配置。"),
    })
}
pub fn validate(name: &str, value: Option<&StoredValue>) -> Result<()> {
    if !CONFIG_NAMES.contains(&name) || value.is_some_and(|v| v.kind != 3 || v.bytes.len() > 65536)
    {
        bail!("视觉配置备份无效。");
    }
    // Snapshots preserve original bytes, including an invalid pre-existing config.
    // Runtime parsing and forward plans validate their own content separately.
    Ok(())
}
pub fn read(name: &str) -> Result<Option<StoredValue>> {
    match fs::read(path(name)?) {
        Ok(bytes) if bytes.len() <= 65536 => Ok(Some(StoredValue { kind: 3, bytes })),
        Ok(_) => bail!("视觉配置超过备份上限，未修改。"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn write(name: &str, value: Option<&StoredValue>) -> Result<()> {
    validate(name, value)?;
    let target = path(name)?;
    if let Some(v) = value {
        fs::create_dir_all(target.parent().unwrap())?;
        let tmp = target.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        transaction::persist_new(&tmp, &v.bytes)?;
        crate::worker::replace_file(&tmp, &target)?;
    } else if let Err(e) = fs::remove_file(target)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        return Err(e.into());
    }
    Ok(())
}
pub fn start_enabled() -> Result<bool> {
    Ok(WindowsRegistry
        .read(&startup_slot())?
        .and_then(|v| registry::as_string(&v))
        .is_some_and(|s| s == start_command().unwrap_or_default()))
}
fn start_command() -> Result<String> {
    Ok(format!("\"{}\" -tray-only", exe()?.display()))
}
fn bytes_entry(name: &str, bytes: Vec<u8>) -> Entry {
    Entry {
        slot: config_slot(name),
        value: Some(StoredValue { kind: 3, bytes }),
    }
}
pub fn start_plan() -> Result<Vec<Entry>> {
    Ok(vec![
        bytes_entry("start-menu.ini", start_preset_with(&appearance()?)),
        bytes_entry(
            "windhawk-settings.ini",
            b"[Settings]\r\nDisableUpdateCheck=1\r\nHideTrayIcon=1\r\nDontAutoShowToolkit=1\r\nDisableToolkitHotkey=1\r\n\r\n[Engine]\r\n".to_vec(),
        ),
        Entry {
            slot: startup_slot(),
            value: Some(registry::string(&start_command()?)),
        },
    ])
}
fn start_preset_with(settings: &Appearance) -> Vec<u8> {
    // Native, theme-aware acrylic; no layout/height changes, preserving accessibility and DPI.
    let brush = format!(
        "<AcrylicBrush TintColor=\"{{ThemeResource SystemAltHighColor}}\" TintOpacity=\"{0:.2}\" TintLuminosityOpacity=\"{0:.2}\" FallbackColor=\"{{ThemeResource SystemAltHighColor}}\"/>",
        f64::from(settings.tint) / 100.0
    );
    let mut text = String::from(
        "[Mod]\r\nLibraryFileName=windows-11-start-menu-styler_1.7.dll\r\nDisabled=0\r\nInclude=StartMenuExperienceHost.exe\r\nIncludeExcludeCustomOnly=0\r\nArchitecture=x86-64\r\nVersion=1.7\r\nLoggingEnabled=0\r\n\r\n[Settings]\r\ntheme=\r\ndisableNewStartMenuLayout=default\r\n",
    );
    for (i, target) in [
        "Border#AcrylicBorder",
        "Border#AppBorder",
        "Border#AccentAppBorder",
    ]
    .iter()
    .enumerate()
    {
        text.push_str(&format!("controlStyles[{i}].target={target}\r\ncontrolStyles[{i}].styles[0]=Background:={brush}\r\ncontrolStyles[{i}].styles[1]=CornerRadius={}\r\n", settings.radius));
    }
    for (index, target) in [
        "TextBlock#Text",
        "TextBlock#UserTileNameText",
        "TextBlock#AllListHeadingText",
        "TextBlock#PinnedListHeaderText",
        "TextBlock#AllAppsHeading",
    ]
    .iter()
    .enumerate()
    {
        let i = index + 3;
        text.push_str(&format!("controlStyles[{i}].target={target}\r\ncontrolStyles[{i}].styles[0]=FontFamily=HarmonyOS Sans SC\r\ncontrolStyles[{i}].styles[1]=FontWeight=SemiBold\r\n"));
    }
    text.into_bytes()
}
pub fn prepare_start() -> Result<()> {
    if !start_supported() {
        bail!("开始菜单美化需要 Windows 11 x64。");
    }
    runtime::preflight(&exe()?)?;
    if format!("{:x}", Sha256::digest(ARCHIVE)) != HASH {
        bail!("开始菜单组件校验失败。");
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(ARCHIVE))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry
            .enclosed_name()
            .ok_or_else(|| anyhow::anyhow!("组件路径无效。"))?
            .to_owned();
        if entry.is_dir() {
            continue;
        }
        if entry.size() > 5_000_000 {
            bail!("组件大小异常。");
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        let target = root()?.join(name);
        fs::create_dir_all(target.parent().unwrap())?;
        if target.exists() {
            if fs::read(target)? != bytes {
                bail!("开始菜单组件被修改，未启动。");
            }
        } else {
            transaction::persist_new(&target, &bytes)?;
        }
    }
    Ok(())
}
pub fn start_running() -> Result<bool> {
    runtime::own_running(&exe()?)
}
pub fn stop_start() -> Result<()> {
    // Windhawk's IPC uses a global daemon. Never send -exit while another copy owns it.
    runtime::preflight(&exe()?)?;
    if !start_running()? {
        return Ok(());
    }
    let mut child = runtime::command(&exe()?)
        .args(["-exit", "-wait", "-timeout", "5000"])
        .spawn()?;
    let until = Instant::now() + Duration::from_secs(7);
    while Instant::now() < until {
        if child.try_wait()?.is_some() && !start_running()? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("开始菜单组件尚未退出，请稍后重试；没有强制结束其他进程。")
}
pub fn sync_start() -> Result<()> {
    if start_enabled()? {
        prepare_start()?;
        if !start_running()? {
            runtime::command(&exe()?).arg("-tray-only").spawn()?;
            let until = Instant::now() + Duration::from_secs(4);
            while Instant::now() < until {
                if start_running()? {
                    return verify_start_loaded();
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            bail!("开始菜单组件没有启动成功。");
        }
        verify_start_loaded()
    } else if start_running()? {
        stop_start()
    } else {
        Ok(())
    }
}
fn verify_start_loaded() -> Result<()> {
    // If Windows hasn't started its menu host yet, Windhawk attaches when it appears.
    if runtime::processes("StartMenuExperienceHost.exe")?.is_empty() {
        return Ok(());
    }
    let until = Instant::now() + Duration::from_secs(15);
    while Instant::now() < until {
        if runtime::loaded_module(
            "StartMenuExperienceHost.exe",
            "windows-11-start-menu-styler_1.7.dll",
        )? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    bail!("开始菜单样式尚未加载，已停止本次启用；不会把进程启动当成美化成功。")
}
pub fn sync() -> Result<()> {
    sync_start()?;
    crate::window_material::sync()
}
pub fn reload() -> Result<()> {
    stop_start()?;
    crate::window_material::pause()?;
    sync()
}
pub fn set_start(enabled: bool) -> Result<crate::models::ActionResult> {
    if enabled {
        prepare_start()?;
    }
    let slots = vec![
        config_slot("start-menu.ini"),
        config_slot("windhawk-settings.ini"),
        startup_slot(),
    ];
    let entries = if enabled {
        start_plan()?
    } else {
        shell_engine::baseline(&slots)?
    };
    let result = shell_engine::commit("visual:start-menu", entries, || {
        stop_start()?;
        sync_start()
    });
    if result.is_err() {
        let _ = sync_start();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_values_are_bounded_and_reach_the_actual_start_preset() {
        let settings = Appearance {
            material: 2,
            tint: 35,
            radius: 22,
        };
        settings.validate().unwrap();
        let preset = String::from_utf8(start_preset_with(&settings)).unwrap();
        assert!(preset.contains("TintOpacity=\"0.35\"") && preset.contains("CornerRadius=22"));
        assert!(
            Appearance {
                tint: 0,
                ..Appearance::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Appearance {
                radius: 25,
                ..Appearance::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Appearance {
                material: 99,
                ..Appearance::default()
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn preset_is_readable_and_does_not_change_shell_layout() {
        let p = String::from_utf8(start_preset_with(&Appearance::default())).unwrap();
        assert!(
            p.contains("TintOpacity=\"0.78\"") && p.contains("disableNewStartMenuLayout=default")
        );
        assert!(p.contains("Include=StartMenuExperienceHost.exe\r\n"));
        assert!(!p.contains("explorer.exe") && !p.contains("Height="));
    }
    #[test]
    fn config_paths_are_closed_and_bounded() {
        assert!(path("../windhawk.ini").is_err());
        assert!(
            validate(
                "start-menu.ini",
                Some(&StoredValue {
                    kind: 3,
                    bytes: vec![0; 65537]
                })
            )
            .is_err()
        );
        assert!(
            validate(
                "material.json",
                Some(&StoredValue {
                    kind: 3,
                    bytes: b"{\"enabled\":true,\"unknown\":1}".to_vec()
                })
            )
            .is_ok()
        );
    }
}
