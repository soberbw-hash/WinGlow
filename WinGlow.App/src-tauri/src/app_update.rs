//! Signed application updates, independent of system customization transactions.
use serde::Serialize;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub current_version: String,
    pub phase: String,
    pub version: Option<String>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
}
struct State {
    view: View,
    pending: Option<(Update, Arc<Vec<u8>>)>,
}
static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| {
    Mutex::new(State {
        view: View {
            current_version: env!("CARGO_PKG_VERSION").into(),
            phase: "idle".into(),
            version: None,
            downloaded: 0,
            total: None,
            error: None,
        },
        pending: None,
    })
});
pub fn view() -> View {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).view.clone()
}
fn active(phase: &str) -> bool {
    matches!(phase, "checking" | "downloading" | "ready" | "installing")
}
fn allowed_download(url: &str, version: &str) -> bool {
    semver::Version::parse(version).is_ok()
        && url
            == format!(
                "https://github.com/soberbw-hash/WinGlow/releases/download/v{version}/WinGlow-{version}-Setup.exe"
            )
}
pub fn check(app: tauri::AppHandle) -> View {
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if active(&state.view.phase) {
            return state.view.clone();
        }
        state.view.phase = "checking".into();
        state.view.error = None;
        state.view.downloaded = 0;
        state.view.total = None;
        state.view.version = None;
    }
    tauri::async_runtime::spawn(async move {
        let result = prepare(&app).await;
        if let Err(error) = result {
            let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
            state.view.phase = "error".into();
            state.view.error = Some(format!("更新检查或下载未完成，请稍后重试。{error}"));
            state.pending = None;
        }
    });
    view()
}
async fn prepare(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let update = app
        .updater_builder()
        .timeout(Duration::from_secs(25))
        .build()?
        .check()
        .await?;
    let Some(mut update) = update else {
        STATE.lock().unwrap_or_else(|e| e.into_inner()).view.phase = "current".into();
        return Ok(());
    };
    if !allowed_download(update.download_url.as_str(), &update.version) {
        anyhow::bail!("更新包来源无效");
    }
    update.timeout = Some(Duration::from_secs(900));
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.view.phase = "downloading".into();
        state.view.version = Some(update.version.clone());
    }
    // The upstream download method verifies the signature before returning bytes.
    // Finished network transfer alone must never enable the installation button.
    let bytes = update
        .download(
            |chunk, total| {
                let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
                state.view.downloaded += chunk as u64;
                state.view.total = total;
            },
            || {},
        )
        .await?;
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.pending = Some((update, Arc::new(bytes)));
    state.view.phase = "ready".into();
    Ok(())
}
pub fn install() -> Result<(), String> {
    let _guard = crate::worker::optimization_lock().map_err(|e| e.to_string())?;
    crate::shell_engine::ensure_ready().map_err(|e| e.to_string())?;
    if !matches!(crate::optimization::state(), Ok((_, false))) {
        return Err("请先完成上次优化的恢复，再安装更新。".into());
    }
    let (update, bytes) = {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if state.view.phase != "ready" {
            return Err("更新包尚未准备好。".into());
        }
        let pending = state.pending.clone().ok_or("更新包尚未准备好。")?;
        state.view.phase = "installing".into();
        state.view.error = None;
        pending
    };
    // Windows updater starts the NSIS /UPDATE /R flow and exits only after
    // successfully launching it. NSIS reinstalls and relaunches WinGlow.
    let install = crate::window_material::pause().and_then(|_| {
        update
            .install(bytes.as_slice())
            .map_err(anyhow::Error::from)
    });
    if let Err(error) = install {
        let recovery = crate::window_material::sync();
        let recovery_note = recovery
            .err()
            .map(|e| format!("；窗口磨砂恢复未完成：{e:#}"))
            .unwrap_or_default();
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.view.phase = "ready".into();
        state.view.error = Some(format!("安装未启动，请重试：{error}{recovery_note}"));
        return Err(format!("安装未启动，请重试：{error}{recovery_note}"));
    }
    Ok(())
}
// Explicit read-only acceptance path: download the published installer even
// when its version equals this executable, verify it, and never install it.
pub async fn diagnose(app: tauri::AppHandle) {
    let result = async {
        let mut update = app.updater_builder().version_comparator(|_, _| true)
            .timeout(Duration::from_secs(25)).build()?.check().await?
            .ok_or_else(|| anyhow::anyhow!("没有公开更新清单"))?;
        if !allowed_download(update.download_url.as_str(), &update.version) { anyhow::bail!("更新包来源无效"); }
        update.timeout = Some(Duration::from_secs(900));
        let bytes = update.download(|_, _| {}, || {}).await?;
        Ok::<_, anyhow::Error>(serde_json::json!({"version":update.version,"verifiedBytes":bytes.len(),"signatureVerified":true,"installed":false}))
    }.await;
    if let Ok(root) = crate::font_engine::data_root() {
        let report = match &result {
            Ok(data) => data.clone(),
            Err(e) => serde_json::json!({"error":e.to_string(),"installed":false}),
        };
        let _ = std::fs::write(root.join("update-verification.json"), report.to_string());
    }
    app.exit(if result.is_ok() { 0 } else { 1 });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_download_is_pinned_to_repository_version_and_installer() {
        assert!(allowed_download(
            "https://github.com/soberbw-hash/WinGlow/releases/download/v1.0.1/WinGlow-1.0.1-Setup.exe",
            "1.0.1"
        ));
        for url in [
            "http://github.com/soberbw-hash/WinGlow/releases/download/v1.0.1/WinGlow-1.0.1-Setup.exe",
            "https://github.com/other/WinGlow/releases/download/v1.0.1/WinGlow-1.0.1-Setup.exe",
            "https://github.com/soberbw-hash/WinGlow/releases/download/v1.0.0/WinGlow-1.0.0-Setup.exe",
        ] {
            assert!(!allowed_download(url, "1.0.1"));
        }
        assert!(!allowed_download(
            "https://github.com/soberbw-hash/WinGlow/releases/download/vbad/WinGlow-bad-Setup.exe",
            "bad"
        ));
    }
    #[test]
    fn duplicate_checks_never_replace_ready_or_installing_payloads() {
        assert!(
            active("checking") && active("downloading") && active("ready") && active("installing")
        );
        assert!(!active("error") && !active("current") && !active("idle"));
    }
}
