//! A real library playlist; no synthetic colour transitions or shell restarts.
use super::*;
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError},
        System::Threading::CreateMutexW,
        UI::Shell::{
            QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
            SHQueryUserNotificationState,
        },
    },
    core::PCWSTR,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Playlist {
    generation: String,
    items: Vec<String>,
    interval_seconds: u32,
}
fn path() -> Result<PathBuf> {
    lively_file("WinGlowCarousel.json")
}
fn read() -> Result<Option<Playlist>> {
    match fs::read(path()?) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
        Ok(bytes) => {
            if bytes.len() > 16384 {
                bail!("壁纸轮播配置过大。");
            }
            let state: Playlist = serde_json::from_slice(&bytes)?;
            if state.items.is_empty()
                || state.items.len() > 100
                || !(3..=1800).contains(&state.interval_seconds)
            {
                bail!("壁纸轮播配置无效。");
            }
            for id in &state.items {
                let name = id.strip_prefix("library:").context("轮播壁纸标识无效。")?;
                if name.is_empty() || name.contains(['\\', '/', '\0']) || matches!(name, "." | "..")
                {
                    bail!("轮播壁纸标识无效。");
                }
            }
            Ok(Some(state))
        }
    }
}
pub(super) fn stop() -> Result<()> {
    match fs::remove_file(path()?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
pub(super) fn active() -> bool {
    read().ok().flatten().is_some()
}
pub(super) fn minutes() -> u32 {
    read()
        .ok()
        .flatten()
        .map(|p| p.interval_seconds / 60)
        .unwrap_or(5)
}
pub(super) fn start_host() -> Result<()> {
    // A restored playlist must reset a surviving host's expected selection.
    // A quick stop/restore can otherwise happen between its polling ticks.
    let Some(mut state) = read()? else {
        return Ok(());
    };
    state.generation = uuid::Uuid::new_v4().to_string();
    save(&state)?;
    runtime::command(&std::env::current_exe()?)
        .arg("--wallpaper-host")
        .spawn()?;
    Ok(())
}
fn save(state: &Playlist) -> Result<()> {
    let temporary = path()?.with_extension(format!("{}.tmp", state.generation));
    transaction::persist_new(&temporary, &serde_json::to_vec(state)?)?;
    worker::replace_file(&temporary, &path()?)
}
fn show(exe: &Path, id: &str) -> Result<()> {
    let project = library_project(id)?;
    send_wallpaper_command(
        exe,
        &[
            "setwp".into(),
            "--file".into(),
            project.to_string_lossy().into_owned(),
        ],
    )?;
    wait_for_selection(exe, &project)
}
pub(super) fn configure(exe: &Path, minutes: u32) -> Result<()> {
    if !matches!(minutes, 1 | 5 | 10 | 15 | 30) {
        bail!("壁纸切换间隔无效。");
    }
    let mut items = wallpaper_library()?
        .items
        .into_iter()
        .filter(|i| i.id.starts_with("library:"))
        .map(|i| i.id)
        .collect::<Vec<_>>();
    // Start with an actual animated wallpaper rather than an artificial colour field.
    items.sort_by_key(|id| {
        library_project(id)
            .ok()
            .and_then(|p| fs::read(p.join("LivelyInfo.json")).ok())
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .map(|v| if v["Title"] == "Rain" { 0 } else { 1 })
            .unwrap_or(2)
    });
    let first = items
        .first()
        .context("没有可用壁纸，未启用轮播。请先导入视频或动图。")?;
    stop()?;
    show(exe, first)?;
    let state = Playlist {
        generation: uuid::Uuid::new_v4().to_string(),
        items,
        interval_seconds: minutes * 60,
    };
    save(&state)?;
    start_host()
}
pub(super) fn first_project() -> Result<PathBuf> {
    let cards = wallpaper_library()?.items;
    let first = cards
        .iter()
        .find(|item| item.title == "Rain" && item.id.starts_with("library:"))
        .or_else(|| cards.iter().find(|item| item.id.starts_with("library:")))
        .context("没有可用的壁纸库项目。")?;
    library_project(&first.id)
}
fn selected() -> Result<Option<String>> {
    let bytes = fs::read(lively_file("WallpaperLayout.json")?)?;
    let layout: serde_json::Value = serde_json::from_slice(&bytes)?;
    Ok(layout
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v["LivelyInfoPath"].as_str())
        .and_then(|p| Path::new(p).file_name())
        .map(|n| format!("library:{}", n.to_string_lossy())))
}
fn next_item(items: &[String], current: &str) -> Option<String> {
    let index = items.iter().position(|id| id == current)?;
    items.get((index + 1) % items.len()).cloned()
}
#[derive(Debug, PartialEq, Eq)]
enum EngineState {
    Refreshing,
    Unavailable,
    ExternalSelection,
    Ready,
}
fn engine_state(
    refresh_pending: bool,
    running: bool,
    selected: Option<&str>,
    expected: &str,
) -> EngineState {
    if refresh_pending {
        EngineState::Refreshing
    } else if !running {
        EngineState::Unavailable
    } else if selected != Some(expected) {
        EngineState::ExternalSelection
    } else {
        EngineState::Ready
    }
}
pub(super) fn host() -> Result<()> {
    let mutex: Vec<u16> = "Local\\WinGlowWallpaperPlaylist"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let handle = unsafe { CreateMutexW(None, true, PCWSTR(mutex.as_ptr())) }?;
    let duplicate = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    struct Guard(windows::Win32::Foundation::HANDLE);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _mutex = Guard(handle);
    if duplicate {
        return Ok(());
    }
    let exe = lively_executable()?;
    ensure_lively_started(&exe)?;
    let mut generation = String::new();
    let mut expected = selected()?.unwrap_or_default();
    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let Some(state) = read()? else {
            return Ok(());
        };
        // The refresh may have finished its registry transaction but still be
        // recovering the engine. Its durable marker owns this pause, not a
        // user request to disable the playlist.
        if wallpaper_refresh_pending()? {
            last = Instant::now();
            continue;
        }
        if generation != state.generation {
            generation = state.generation;
            expected = selected()?.unwrap_or_default();
            last = Instant::now();
        }
        if last.elapsed() < Duration::from_secs(state.interval_seconds.into()) {
            continue;
        }
        let Ok(_guard) = worker::optimization_lock() else {
            continue;
        };
        let notification = unsafe { SHQueryUserNotificationState() }?;
        if [
            QUNS_BUSY,
            QUNS_RUNNING_D3D_FULL_SCREEN,
            QUNS_PRESENTATION_MODE,
        ]
        .contains(&notification)
        {
            last = Instant::now();
            continue;
        }
        // Re-read after acquiring the mutation lock: manual selection and restore
        // must win over an already-due timer. Do not overwrite choices in Lively.
        let Some(current) = read()? else {
            return Ok(());
        };
        if current.generation != generation {
            continue;
        }
        let pending = wallpaper_refresh_pending()?;
        let running = runtime::own_running(&exe)?;
        let selection = if running && !pending {
            selected()?
        } else {
            None
        };
        match engine_state(pending, running, selection.as_deref(), &expected) {
            EngineState::Refreshing => {
                last = Instant::now();
                continue;
            }
            EngineState::Unavailable => {
                // Do not restart a manually closed engine, or erase a saved
                // playlist merely because the engine crashed. A later normal
                // launch can resume it.
                fs::write(font_engine::data_root()?.join("wallpaper-carousel-status.json"),
                    serde_json::json!({"selected":expected,"success":false,"error":"壁纸播放器已退出，轮播配置已保留。"}).to_string())?;
                return Ok(());
            }
            EngineState::ExternalSelection => {
                stop()?;
                return Ok(());
            }
            EngineState::Ready => (),
        }
        let next = next_item(&current.items, &expected).context("当前壁纸已不在轮播列表中。")?;
        let result = show(&exe, &next);
        let report = serde_json::json!({"selected":next,"success":result.is_ok(),"error":result.as_ref().err().map(|e|format!("{e:#}"))});
        fs::write(
            font_engine::data_root()?.join("wallpaper-carousel-status.json"),
            report.to_string(),
        )?;
        result?;
        expected = next;
        last = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refresh_pause_is_not_a_request_to_remove_the_playlist() {
        assert_eq!(
            engine_state(true, false, None, "library:a"),
            EngineState::Refreshing
        );
        assert_eq!(
            engine_state(true, true, Some("library:b"), "library:a"),
            EngineState::Refreshing
        );
        assert_eq!(
            engine_state(false, false, None, "library:a"),
            EngineState::Unavailable
        );
        assert_eq!(
            engine_state(false, true, Some("library:b"), "library:a"),
            EngineState::ExternalSelection
        );
        assert_eq!(
            engine_state(false, true, Some("library:a"), "library:a"),
            EngineState::Ready
        );
    }
    #[test]
    fn playlist_wraps_and_does_not_override_external_selection() {
        let ids = vec!["library:a".into(), "library:b".into(), "library:c".into()];
        assert_eq!(next_item(&ids, "library:a").as_deref(), Some("library:b"));
        assert_eq!(next_item(&ids, "library:c").as_deref(), Some("library:a"));
        assert_eq!(next_item(&ids, "library:external"), None);
    }
}
