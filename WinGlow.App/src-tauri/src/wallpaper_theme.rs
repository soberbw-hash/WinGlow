//! Keep Windows taskbar/system text readable over dark live wallpapers.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, io::ErrorKind};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    },
};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_READ},
};

const PERSONALIZE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const MARKER_FILE: &str = "wallpaper-theme-before.json";
const DARK_MODE: u32 = 0;
const LIGHT_MODE: u32 = 1;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeState {
    pub system_uses_light_theme: Option<u32>,
    pub apps_use_light_theme: Option<u32>,
}

fn marker_path() -> Result<std::path::PathBuf> {
    Ok(crate::font_engine::data_root()?.join(MARKER_FILE))
}

fn personalize_key(read_only: bool) -> Result<RegKey> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    if read_only {
        root.open_subkey_with_flags(PERSONALIZE_KEY, KEY_READ)
            .map_err(Into::into)
    } else {
        root.create_subkey(PERSONALIZE_KEY)
            .map(|(key, _)| key)
            .map_err(Into::into)
    }
}

fn read_value(key: &RegKey, name: &str) -> Result<Option<u32>> {
    match key.get_value(name) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("无法读取 Windows 颜色设置 {name}。")),
    }
}

pub fn capture() -> Result<ThemeState> {
    match personalize_key(true) {
        Ok(key) => Ok(ThemeState {
            system_uses_light_theme: read_value(&key, "SystemUsesLightTheme")?,
            apps_use_light_theme: read_value(&key, "AppsUseLightTheme")?,
        }),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == ErrorKind::NotFound) =>
        {
            Ok(ThemeState::default())
        }
        Err(error) => Err(error),
    }
}

fn write_value(key: &RegKey, name: &str, value: Option<u32>) -> Result<()> {
    match value {
        Some(value) => key.set_value(name, &value)?,
        None => match key.delete_value(name) {
            Ok(()) => (),
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        },
    }
    Ok(())
}

fn write_values(state: &ThemeState) -> Result<()> {
    let key = personalize_key(false)?;
    write_value(&key, "SystemUsesLightTheme", state.system_uses_light_theme)?;
    write_value(&key, "AppsUseLightTheme", state.apps_use_light_theme)?;
    Ok(())
}

fn write(state: &ThemeState) -> Result<()> {
    let before = capture()?;
    if let Err(error) = write_values(state) {
        return match write_values(&before) {
            Ok(()) => Err(error),
            Err(rollback) => Err(anyhow::anyhow!(
                "{error:#}；Windows 颜色设置回滚也未完成：{rollback:#}"
            )),
        };
    }
    notify_windows_theme();
    Ok(())
}

fn persist_marker(state: &ThemeState) -> Result<()> {
    let path = marker_path()?;
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    crate::transaction::persist_new(&temp, &serde_json::to_vec(state)?)?;
    crate::worker::replace_file(&temp, &path)
}

fn marker_state() -> Result<Option<ThemeState>> {
    let path = marker_path()?;
    match fs::read(path) {
        Ok(bytes) if bytes.len() <= 8192 => Ok(Some(serde_json::from_slice(&bytes)?)),
        Ok(_) => anyhow::bail!("Windows 颜色恢复记录过大。"),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn baseline() -> Result<Option<ThemeState>> {
    marker_state()
}

fn set_for_wallpaper(dark: bool) -> Result<()> {
    let current_marker = marker_state()?;
    if current_marker.is_none() {
        persist_marker(&capture()?)?;
    }
    let result = write(&ThemeState {
        system_uses_light_theme: Some(if dark { DARK_MODE } else { LIGHT_MODE }),
        apps_use_light_theme: Some(LIGHT_MODE),
    });
    if let Err(error) = result {
        if current_marker.is_none() {
            let _ = fs::remove_file(marker_path()?);
        }
        return Err(error);
    }
    Ok(())
}

pub fn use_dark_wallpaper_mode() -> Result<()> {
    set_for_wallpaper(true)
}

pub fn use_light_wallpaper_mode() -> Result<()> {
    set_for_wallpaper(false)
}

pub fn restore_baseline() -> Result<()> {
    let Some(state) = marker_state()? else {
        return Ok(());
    };
    write(&state)?;
    fs::remove_file(marker_path()?)?;
    Ok(())
}

pub fn restore_snapshot(current: Option<&ThemeState>, baseline: Option<&ThemeState>) -> Result<()> {
    if let Some(current) = current {
        write(current)?;
    }
    let path = marker_path()?;
    match baseline {
        Some(baseline) => persist_marker(baseline)?,
        None => match fs::remove_file(path) {
            Ok(()) => (),
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        },
    }
    Ok(())
}

fn notify_windows_theme() {
    let name = "ImmersiveColorSet"
        .encode_utf16()
        .chain([0])
        .collect::<Vec<_>>();
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(name.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
    }
}
