mod app_update;
mod archive_filter;
mod breeze;
mod breeze_profile;
mod component_runtime;
mod desktop;
mod desktop_access;
mod desktop_settings;
mod explorer;
mod font_engine;
mod legacy;
mod lively_rpc;
mod menu;
mod menu_diagnostics;
mod menu_dictionary;
mod menu_icon;
mod menu_policy;
mod models;
mod normal_restore;
mod optimization;
mod optional_tools;
mod preset_data;
mod registry;
mod repair;
mod shell_engine;
mod taskbar;
mod transaction;
mod tray;
mod ui_fonts;
mod visual;
mod visual_diagnostics;
mod wallpaper_theme;
mod window_layout;
mod window_material;
mod worker;

use models::{ActionResult, BootstrapPayload, Operation};

#[tauri::command]
async fn load_bootstrap() -> Result<BootstrapPayload, String> {
    tauri::async_runtime::spawn_blocking(font_engine::load_bootstrap)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn apply_font(preset_id: String, paths: Vec<String>) -> Result<ActionResult, String> {
    preset_data::find_preset(&preset_id).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Apply { preset_id, paths }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn restore_fonts(mode: String) -> Result<ActionResult, String> {
    if mode != "last" && mode != "default" {
        return Err("未知恢复操作。".into());
    }
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Restore { mode }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn import_fonts(paths: Vec<String>) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Import { paths }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn load_shell() -> Result<models::ShellState, String> {
    tauri::async_runtime::spawn_blocking(shell_engine::load)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn set_tweak(id: String, enabled: bool) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Toggle { id, enabled }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn set_menu_item(id: String, enabled: bool) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Menu { id, enabled }))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn set_appearance(id: String, settings: visual::Appearance) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        worker::run(Operation::Appearance { id, settings })
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
async fn restore_category(category: String) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        worker::run(Operation::RestoreCategory { category })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn recover_pending() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(|| recover_pending_inner(true))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("{error:#}"))
}
pub(crate) fn recover_pending_inner(allow_refresh: bool) -> anyhow::Result<ActionResult> {
    let mut restored = None;
    if optimization::state()?.1 {
        restored = Some(optimization::restore_deferred()?);
    }
    if !transaction::pending_directories(&font_engine::backup_root()?)?.is_empty() {
        restored = Some(worker::run_deferred(Operation::RestoreCategory {
            category: "pending".into(),
        })?);
    }
    let refresh = allow_refresh && restored.is_some();
    explorer::finish(
        Ok(restored.unwrap_or(ActionResult {
            message: "没有未完成的修改，状态已重新检查。".into(),
        })),
        refresh,
        explorer::restart,
    )
}

#[tauri::command]
async fn repair_system() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || worker::run(Operation::Repair))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
fn repair_progress() -> Result<models::RepairProgress, String> {
    repair::progress().map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn optimize_system(restore: bool) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if restore {
            optimization::restore()
        } else {
            optimization::apply()
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
fn optimize_progress() -> optimization::ApplyProgress {
    optimization::apply_progress()
}
#[tauri::command]
async fn restore_normal() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(normal_restore::run)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn optimize_menu() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(|| worker::run(Operation::OptimizeMenu))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

fn homepage_url(kind: &str) -> anyhow::Result<&'static str> {
    match kind {
        "author" => Ok("https://github.com/soberbw-hash"),
        "project" => Ok("https://github.com/soberbw-hash/WinGlow"),
        _ => anyhow::bail!("未知主页入口。"),
    }
}
#[tauri::command]
fn open_homepage(app: tauri::AppHandle, kind: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = homepage_url(&kind).map_err(|e| e.to_string())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod homepage_tests {
    #[test]
    fn homepage_links_are_fixed_and_unknown_targets_are_rejected() {
        assert_eq!(
            super::homepage_url("author").unwrap(),
            "https://github.com/soberbw-hash"
        );
        assert_eq!(
            super::homepage_url("project").unwrap(),
            "https://github.com/soberbw-hash/WinGlow"
        );
        assert!(super::homepage_url("https://example.com").is_err());
    }
}

#[tauri::command]
async fn open_backup(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = font_engine::backup_root().map_err(|e| format!("无法定位备份：{e:#}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法打开备份目录：{e}"))?;
    app.opener()
        .open_path(dir.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| format!("无法打开备份文件夹：{e}"))
}

#[tauri::command]
async fn open_breeze_config() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(breeze::open_config_window)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))?;
    Ok(ActionResult {
        message: "已打开 Breeze Config 可视化设置窗口。".into(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if worker::handle_cli() {
        return;
    }
    // Worker / material-host modes were handled above; adapt only the interactive GUI.
    if desktop_access::initialize() {
        return;
    }
    if desktop_access::handle_probe() {
        return;
    }
    let mut builder = tauri::Builder::default();
    if !std::env::args().any(|a| matches!(a.as_str(), "--verify-updater" | "--verify-tray")) {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
            tray::show(app)
        }));
    }
    builder
        .setup(|app| {
            if std::env::args().any(|arg| arg == "--verify-updater") {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
                tauri::async_runtime::spawn(app_update::diagnose(app.handle().clone()));
                return Ok(());
            }
            tray::setup(app)?;
            if tray::probe(app) {
                return Ok(());
            }
            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(error) = window_layout::fit(&window) {
                        eprintln!("窗口尺寸适配未完成：{error}");
                    }
                    let current = window.clone();
                    window.on_window_event(move |event| {
                        if matches!(event, tauri::WindowEvent::ScaleFactorChanged { .. })
                            && let Err(error) = window_layout::fit(&current)
                        {
                            eprintln!("窗口缩放适配未完成：{error}");
                        }
                    });
                }
            }
            explorer::watch_shell();
            std::thread::spawn(|| {
                let result = optimization::update_on_startup()
                    .and_then(|upgrade| resume_desktop_components().map(|_| upgrade));
                // Keep mutation controls locked until runtime restoration completes too.
                optimization::finish_upgrade(&result);
                if let Err(error) = result {
                    eprintln!("桌面初始化未完成：{error:#}");
                }
            });
            app_update::check(app.handle().clone());
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            load_bootstrap,
            apply_font,
            restore_fonts,
            import_fonts,
            load_shell,
            set_tweak,
            set_appearance,
            set_menu_item,
            restore_category,
            recover_pending,
            repair_system,
            repair_progress,
            optimize_system,
            optimize_progress,
            optimize_menu,
            restore_normal,
            open_backup,
            open_breeze_config,
            open_homepage,
            update_status,
            check_updates,
            install_update,
            optional_tool,
            wallpaper_control,
            load_desktop_settings,
            apply_taskbar_settings,
            select_wallpaper,
            set_wallpaper_volume,
            choose_wallpaper,
            optimization_upgrade_status,
            retry_optimization_upgrade,
        ])
        .run(tauri::generate_context!())
        .expect("启动 WinGlow失败");
}

fn resume_desktop_components() -> anyhow::Result<()> {
    let _guard = worker::optimization_lock()?;
    shell_engine::ensure_ready()?;
    optional_tools::stop_legacy_rotation()?;
    if optimization::state()?.0 {
        font_engine::repair_ui_font_sizes()?;
    }
    ui_fonts::synchronize_desktop()?;
    if optimization::state()?.1 {
        return Ok(());
    }
    if breeze::enabled()? {
        let entries = archive_filter::plan()?;
        if !entries.is_empty() {
            shell_engine::commit("menu:sync-disabled-display", entries, || Ok(()))?;
        }
        // The enabled preference can outlive an exited injector. Reuse our
        // existing runtime or start it after synchronizing the stable rules.
        breeze::sync()?;
    }
    visual::sync()?;
    optional_tools::resume_lively_after_refresh()
}

#[tauri::command]
fn update_status() -> app_update::View {
    app_update::view()
}
#[tauri::command]
fn check_updates(app: tauri::AppHandle) -> app_update::View {
    app_update::check(app)
}
#[tauri::command]
async fn install_update() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(app_update::install)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn optional_tool(id: String, action: String) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || optional_tools::run(id, action))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn wallpaper_control(action: String) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || optional_tools::wallpaper_control(&action, None))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSettings {
    taskbar: desktop_settings::TaskbarSettings,
    wallpaper: optional_tools::WallpaperLibrary,
}
#[tauri::command]
async fn load_desktop_settings() -> Result<DesktopSettings, String> {
    tauri::async_runtime::spawn_blocking(|| {
        Ok::<_, anyhow::Error>(DesktopSettings {
            taskbar: desktop_settings::taskbar_settings()?,
            wallpaper: optional_tools::wallpaper_library()?,
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn apply_taskbar_settings(
    settings: desktop_settings::TaskbarSettings,
) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        worker::run(Operation::TaskbarSettings { settings })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn select_wallpaper(id: String, minutes: u32) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || optional_tools::select_wallpaper(id, minutes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
async fn set_wallpaper_volume(volume: u8) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || optional_tools::set_wallpaper_volume(volume))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
fn optimization_upgrade_status() -> optimization::UpgradeStatus {
    optimization::upgrade_status()
}
#[tauri::command]
async fn retry_optimization_upgrade() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let result = optimization::update_if_due()
            .and_then(|upgrade| resume_desktop_components().map(|_| upgrade));
        optimization::finish_upgrade(&result);
        result.map(|result| {
            result.unwrap_or(ActionResult {
                message: "桌面初始化已完成，当前优化方案已是最新。".into(),
            })
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn choose_wallpaper(app: tauri::AppHandle) -> Result<ActionResult, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("视频和动图", &["mp4", "webm", "gif"])
            .blocking_pick_file()
        else {
            return Ok(ActionResult {
                message: String::new(),
            });
        };
        let path = file.into_path().map_err(|e| e.to_string())?;
        optional_tools::wallpaper_control("choose", Some(path)).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
