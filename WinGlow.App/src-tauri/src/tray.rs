use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
pub fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开 WinGlow", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "退出 WinGlow", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &exit])?;
    let mut builder = TrayIconBuilder::with_id("winglow")
        .tooltip("WinGlow · 后台运行")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(app),
            "exit" => {
                // Hide/exit must not abandon an in-flight system change.
                if !crate::optimization::upgrade_status().running
                    && let Ok(_guard) = crate::worker::optimization_lock()
                {
                    app.exit(0);
                } else {
                    show(app);
                }
            }
            _ => (),
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    if let Some(window) = app.get_webview_window("main") {
        let current = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = current.hide();
            }
        });
    }
    Ok(())
}
pub fn probe(app: &tauri::App) -> bool {
    if !std::env::args().any(|a| a == "--verify-tray") {
        return false;
    }
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            let window = handle.get_webview_window("main").unwrap();
            anyhow::ensure!(
                handle.tray_by_id("winglow").is_some(),
                "Tray not registered"
            );
            window.close()?;
            std::thread::sleep(std::time::Duration::from_millis(350));
            anyhow::ensure!(!window.is_visible()?, "Closing did not hide the window");
            show(&handle);
            std::thread::sleep(std::time::Duration::from_millis(350));
            anyhow::ensure!(window.is_visible()?, "Window could not reopen");
            Ok(())
        })();
        if let Ok(root) = crate::font_engine::data_root() {
            let _ = std::fs::write(
                root.join("tray-verification.json"),
                serde_json::json!({
                "success":result.is_ok(),"error":result.as_ref().err().map(|e|format!("{e:#}"))})
                .to_string(),
            );
        }
        handle.exit(if result.is_ok() { 0 } else { 1 });
    });
    true
}
