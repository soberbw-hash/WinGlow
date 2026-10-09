//! Explicit opt-in local verification; never runs during normal app launch.
use crate::{
    component_runtime, font_engine,
    registry::{self, Entry, Scope, WindowsRegistry},
    shell_engine,
    transaction::ValueStore,
    visual, window_material,
};
use anyhow::{Result, bail};
use std::{
    fs,
    time::{Duration, Instant},
};

pub fn run() -> Result<()> {
    let _guard = crate::worker::optimization_lock()?;
    shell_engine::ensure_ready()?;
    let slots = [
        visual::config_slot("start-menu.ini"),
        visual::config_slot("windhawk-settings.ini"),
        registry::slot(Scope::Startup, "WinGlow-StartMenu"),
        visual::config_slot("material.json"),
        registry::slot(Scope::Startup, "WinGlow-WindowMaterial"),
    ];
    let before = slots
        .iter()
        .map(|slot| {
            Ok(Entry {
                slot: slot.clone(),
                value: WindowsRegistry.read(slot)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let result = (|| {
        visual::set_start(true)?;
        let until = Instant::now() + Duration::from_secs(25);
        let mut loaded = false;
        while Instant::now() < until {
            loaded = component_runtime::loaded_module(
                "StartMenuExperienceHost.exe",
                "windows-11-start-menu-styler_1.7.dll",
            )?;
            if loaded {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        if !loaded {
            let engine =
                component_runtime::loaded_module("StartMenuExperienceHost.exe", "windhawk.dll")?;
            bail!("开始菜单样式 DLL 未加载（引擎加载={engine}）；启动进程不算效果验证成功。");
        }
        let settings = visual::Appearance {
            material: 2,
            tint: 35,
            radius: 22,
        };
        visual::apply_settings("start-menu", settings)?;
        let actual = visual::appearance()?;
        if actual.tint != 35 || actual.radius != 22 {
            bail!("开始菜单自定义参数没有保存。");
        }
        window_material::set_style(2)?;
        let mica_host = window_material::verify_host()?;
        window_material::set_style(3)?;
        window_material::set(true)?;
        let backdrop = window_material::verify_api()?;
        let host = window_material::verify_host()?;
        Ok(
            serde_json::json!({"startMenuModuleLoaded":loaded,"customTint":actual.tint,"customRadius":actual.radius,"micaHostLifecycle":mica_host,"backdropApi":backdrop,"hostLifecycle":host,"build":visual::build()}),
        )
    })();
    let restore = shell_engine::commit("verify:visual-restore", before.clone(), visual::reload);
    let exact = restore.is_ok()
        && before.iter().all(|entry| {
            WindowsRegistry
                .read(&entry.slot)
                .is_ok_and(|v| v == entry.value)
        });
    let mut report = serde_json::json!({"restoredExactly":exact,"success":result.is_ok()&&exact});
    match result {
        Ok(value) => report["checks"] = value,
        Err(e) => report["error"] = serde_json::json!(format!("{e:#}")),
    };
    if let Err(e) = restore {
        report["restoreError"] = serde_json::json!(format!("{e:#}"));
    }
    fs::write(
        font_engine::data_root()?.join("visual-verification.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    if report["success"] != true {
        bail!("视觉组件验证未通过，请查看 visual-verification.json。");
    }
    Ok(())
}
