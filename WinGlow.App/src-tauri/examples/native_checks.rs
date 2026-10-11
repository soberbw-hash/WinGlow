//! Separate acceptance executable so a running material host is not replaced.
//! --finish-one-click-effects is explicit real-desktop acceptance only.
include!("../src/lib.rs");
fn main() {
    if std::env::args().any(|arg| arg == "--resume-breeze") {
        let result = breeze::sync();
        println!(
            "{}",
            match result {
                Ok(()) =>
                    serde_json::json!({"success":true,"running":breeze::running().unwrap_or(false)}),
                Err(error) => serde_json::json!({"success":false,"error":format!("{error:#}")}),
            }
        );
        return;
    }
    if std::env::args().any(|arg| arg == "--verify-wallpaper-toggle") {
        let result = (|| -> anyhow::Result<serde_json::Value> {
            let before = optional_tools::states()?;
            anyhow::ensure!(
                before[0].enabled,
                "Wallpaper must already be active for this reversible acceptance"
            );
            let disable = optional_tools::run("lively".into(), "disable".into());
            let off = optional_tools::states()?[0].enabled;
            let enable = optional_tools::run("lively".into(), "enable".into());
            disable?;
            enable?;
            let on = optional_tools::states()?[0].enabled;
            anyhow::ensure!(
                !off && on,
                "Wallpaper switch did not reach the requested state"
            );
            Ok(serde_json::json!({"success":true,"disabled":!off,"enabledAgain":on}))
        })();
        println!(
            "{}",
            match result {
                Ok(value) => value,
                Err(error) => serde_json::json!({"success":false,"error":format!("{error:#}")}),
            }
        );
        return;
    }
    if std::env::args().any(|arg| arg == "--verify-menu-toggle") {
        let result = (|| -> anyhow::Result<serde_json::Value> {
            use transaction::ValueStore;
            shell_engine::ensure_ready()?;
            let (item, _) = menu::scan_impl(false)?
                .into_iter()
                .find(|(item, _)| item.label.contains("WorkBuddy"))
                .ok_or_else(|| anyhow::anyhow!("WorkBuddy test target is not installed"))?;
            let mut entries = menu::resolve(&item.id, !item.enabled)?;
            entries.extend(archive_filter::plan_with(&entries)?);
            let before = entries
                .into_iter()
                .map(|entry| {
                    Ok(registry::Entry {
                        value: registry::WindowsRegistry.read(&entry.slot)?,
                        slot: entry.slot,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let tested = (|| -> anyhow::Result<Vec<String>> {
                let mut messages = Vec::new();
                for enabled in [!item.enabled, item.enabled, item.enabled] {
                    let result = worker::run(models::Operation::Menu {
                        id: item.id.clone(),
                        enabled,
                    })?;
                    anyhow::ensure!(
                        !result.message.contains("资源管理器已刷新"),
                        "Unexpected desktop restart"
                    );
                    messages.push(result.message);
                }
                Ok(messages)
            })();
            let _guard = worker::optimization_lock()?;
            shell_engine::commit("verify:menu-toggle-restore", before.clone(), || Ok(()))?;
            for entry in before {
                anyhow::ensure!(
                    registry::WindowsRegistry.read(&entry.slot)? == entry.value,
                    "Menu setting was not exactly restored"
                );
            }
            Ok(
                serde_json::json!({"success":true,"label":item.label,"originalEnabled":item.enabled,"restoredExactly":true,"messages":tested?}),
            )
        })();
        println!(
            "{}",
            match result {
                Ok(value) => value,
                Err(error) => serde_json::json!({"success":false,"error":format!("{error:#}")}),
            }
        );
        return;
    }
    if std::env::args().any(|arg| arg == "--verify-taskbar") {
        let result = (|| -> anyhow::Result<serde_json::Value> {
            use transaction::ValueStore;
            let _guard = worker::optimization_lock()?;
            shell_engine::ensure_ready()?;
            let before = taskbar::plan()?
                .into_iter()
                .map(|entry| {
                    Ok(registry::Entry {
                        value: registry::WindowsRegistry.read(&entry.slot)?,
                        slot: entry.slot,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let tested = (|| -> anyhow::Result<_> {
                let mut settings = desktop_settings::TaskbarSettings::default();
                settings.desktop.accent = "opaque".into();
                settings.desktop.color = "#185A94".into();
                settings.desktop.opacity = 100;
                settings.maximized = Some(settings.desktop.clone());
                settings.start_open = Some(settings.desktop.clone());
                desktop_settings::apply_taskbar(settings)?;
                let bytes = taskbar::read_config()?.unwrap().bytes;
                let json: serde_json::Value = serde_json::from_slice(&bytes)?;
                for key in [
                    "desktop_appearance",
                    "visible_window_appearance",
                    "maximized_window_appearance",
                    "start_opened_appearance",
                    "search_opened_appearance",
                    "task_view_opened_appearance",
                ] {
                    anyhow::ensure!(
                        json[key]["accent"] == "opaque" && json[key]["color"] == "#185A94FF",
                        "Scene did not receive settings: {key}"
                    );
                }
                Ok(json)
            })();
            taskbar::pause_owned()?;
            shell_engine::commit("verify:taskbar-restore", before.clone(), taskbar::sync)?;
            for entry in before {
                anyhow::ensure!(
                    registry::WindowsRegistry.read(&entry.slot)? == entry.value,
                    "Original taskbar setting was not restored"
                );
            }
            Ok(serde_json::json!({"success":true,"restoredExactly":true,"applied":tested?}))
        })();
        println!(
            "{}",
            match result {
                Ok(value) => value,
                Err(error) => serde_json::json!({"success":false,"error":format!("{error:#}")}),
            }
        );
        return;
    }
    if let Some(index) = std::env::args().position(|arg| arg == "--apply-font") {
        let preset_id = std::env::args().nth(index + 1).unwrap();
        let result = worker::run_deferred(models::Operation::Apply {
            preset_id,
            paths: vec![],
        });
        println!("{:?}", result.map(|value| value.message));
        println!("{}", ui_fonts::diagnostics().unwrap());
        return;
    }
    if let Some(index) = std::env::args().position(|arg| arg == "--select-wallpaper") {
        let id = std::env::args().nth(index + 1).unwrap();
        let result = optional_tools::select_wallpaper(id, 1);
        println!("{:?}", result.map(|value| value.message));
        return;
    }
    assert!(
        std::env::args()
            .skip(1)
            .any(|argument| argument.starts_with("--")),
        "Pass a diagnostic worker argument"
    );
    let finish_effects = std::env::args().any(|argument| argument == "--finish-one-click-effects");
    let wallpaper_only = std::env::args().any(|argument| argument == "--apply-wallpaper-default");
    if finish_effects || wallpaper_only {
        let result = (|| -> anyhow::Result<()> {
            let _guard = worker::optimization_lock()?;
            shell_engine::ensure_ready()?;
            desktop_access::ensure_compatible()?;
            let before = optional_tools::capture_wallpaper_optimization()?;
            let refresh = if finish_effects {
                explorer::restart_for_optimization()
            } else {
                Ok(())
            };
            let outcome = refresh
                .and_then(|()| optional_tools::apply_wallpaper_optimization())
                .and_then(|()| ui_fonts::verify_desktop_font());
            if let Err(error) = outcome {
                optional_tools::restore_wallpaper_optimization(&before)
                    .map_err(|restore| anyhow::anyhow!("{error:#}; {restore:#}"))?;
                return Err(error);
            }
            Ok(())
        })();
        let report = match result {
            Ok(()) => serde_json::json!({"success":true}),
            Err(error) => serde_json::json!({"success":false,"error":format!("{error:#}")}),
        };
        println!("{report}");
        return;
    }
    run();
}
