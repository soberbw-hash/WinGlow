//! Opt-in upstream applications, installed only when the user selects Install.
//! Keep external installations separate: never uninstall an instance WinGlow didn't install.
use crate::{component_runtime as runtime, font_engine, models::ActionResult, transaction, worker};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use winreg::{RegKey, enums::*};
mod carousel;
pub fn carousel_host() -> Result<()> {
    carousel::host()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolState {
    pub id: String,
    pub label: String,
    pub installed: bool,
    pub enabled: bool,
    pub managed: bool,
    pub supported: bool,
    pub note: String,
}
struct Tool {
    id: &'static str,
    label: &'static str,
    version: &'static str,
    url: &'static str,
    hash: &'static str,
    size: usize,
}
const LIVELY: Tool = Tool {
    id: "lively",
    label: "动态壁纸",
    version: "2.2.1.0",
    url: "https://github.com/lively-community/lively/releases/download/v2.2.1.0/lively_setup_x86_full_v2210.exe",
    hash: "98f4e96bb8e2c416384eeaf48016eadaea9dce8263b8d212052775ebcf2d7e34",
    size: 218063717,
};
fn tool(id: &str) -> Result<&'static Tool> {
    if id == "lively" {
        Ok(&LIVELY)
    } else {
        bail!("未知可选工具。")
    }
}
fn root(t: &Tool) -> Result<PathBuf> {
    Ok(font_engine::data_root()?
        .join("Optional")
        .join(t.id)
        .join(t.version))
}
fn marker(t: &Tool) -> Result<PathBuf> {
    Ok(root(t)?.join("installation.json"))
}

fn own_exe(t: &Tool) -> Result<PathBuf> {
    Ok(root(t)?.join("Application/Lively.exe"))
}
fn lively_existing() -> Option<PathBuf> {
    let key = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{E3E43E1B-DEC8-44BF-84A6-243DBA3F2CB1}_is1";
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        if let Ok(key) = RegKey::predef(hive).open_subkey(key)
            && let Ok(dir) = key.get_value::<String, _>("InstallLocation")
        {
            let exe = PathBuf::from(dir).join("Lively.exe");
            if exe.is_absolute() && exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}
// The desktop refresh owns only a temporary runtime interruption. Wallpaper
// selection, layout and preferences remain in Lively's own saved files.
pub struct WallpaperRefresh {
    exe: Option<PathBuf>,
}
fn resume_marker() -> Result<PathBuf> {
    Ok(font_engine::data_root()?.join("lively-refresh-resume.json"))
}
fn known_lively(exe: &Path) -> Result<bool> {
    Ok(runtime::same_path(exe, &own_exe(&LIVELY)?)
        || lively_existing().is_some_and(|installed| runtime::same_path(exe, &installed)))
}
impl WallpaperRefresh {
    pub fn capture() -> Result<Self> {
        let paths = runtime::processes("Lively.exe")?;
        for path in &paths {
            if !known_lively(path)? {
                bail!("无法确认动态壁纸的安装来源，未刷新桌面。请先退出该壁纸程序后重试。");
            }
        }
        if let Some(exe) = paths.first() {
            runtime::preflight(exe)?;
        }
        Ok(Self {
            exe: paths.into_iter().next(),
        })
    }
    pub fn pause(&self) -> Result<()> {
        let Some(exe) = &self.exe else {
            return Ok(());
        };
        backup(&LIVELY)?;
        let marker = resume_marker()?;
        if !marker.exists() {
            transaction::persist_new(&marker, &serde_json::to_vec(exe)?)?;
        }
        runtime::preflight(exe)?;
        if !runtime::own_running(exe)? {
            return Ok(());
        }
        // Lively quits in the server method's finally block; the pipe may close
        // before its Empty response is sent. For shutdown only, observe the
        // original owned process exiting instead of treating lost ACK as failure.
        let shutdown = crate::lively_rpc::shutdown();
        let deadline = Instant::now() + Duration::from_secs(8);
        while runtime::own_running(exe)? {
            if Instant::now() >= deadline {
                shutdown.context("动态壁纸退出未完成，未刷新桌面。")?;
                bail!("动态壁纸尚未退出，未刷新桌面。");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if runtime::own_running(exe)? {
            bail!("动态壁纸尚未退出，未刷新桌面。");
        }
        Ok(())
    }
}
pub fn resume_lively_after_refresh() -> Result<()> {
    let marker = resume_marker()?;
    let bytes = match fs::read(&marker) {
        Ok(bytes) if bytes.len() <= 8192 => bytes,
        Ok(_) => bail!("动态壁纸恢复记录过大。"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if carousel::active() && runtime::own_running(&lively_executable()?)? {
                carousel::start_host()?;
            }
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let exe: PathBuf = serde_json::from_slice(&bytes)?;
    if !known_lively(&exe)? || !exe.is_file() {
        bail!("动态壁纸安装位置已变化，未自动启动。请手动打开动态壁纸。");
    }
    runtime::preflight(&exe)?;
    if !runtime::own_running(&exe)? {
        let mut child = runtime::command(&exe)
            // Fresh upstream instances parse only screensaver arguments. Regular
            // controls are for an already running instance, so start without args.
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            std::thread::sleep(Duration::from_millis(200));
            if let Some(status) = child.try_wait()? {
                if !status.success() || !runtime::own_running(&exe)? {
                    bail!("动态壁纸恢复失败：{status}。请手动打开动态壁纸。");
                }
                break;
            }
            if runtime::own_running(&exe)?
                && deadline.saturating_duration_since(Instant::now()) <= Duration::from_secs(6)
            {
                break;
            }
            if Instant::now() >= deadline {
                bail!("动态壁纸恢复超时，请手动打开动态壁纸。");
            }
        }
    }
    // Keep the playlist intent while its engine was temporarily paused. Reset
    // the timer only after recovery; a surviving host may still hold the lock.
    if carousel::active() {
        carousel::start_host()?;
    }
    fs::remove_file(marker)?;
    Ok(())
}
fn installed(t: &Tool) -> Result<bool> {
    Ok(own_exe(t)?.is_file() || t.id == "lively" && lively_existing().is_some())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Installation {
    version: u32,
    tool_version: String,
    backup: String,
    installed_by_winglow: bool,
}
fn record(t: &Tool) -> Result<Option<Installation>> {
    match fs::read(marker(t)?) {
        Ok(b) if b.len() <= 4096 => {
            let r: Installation = serde_json::from_slice(&b)?;
            if r.version != 1 || r.tool_version != t.version {
                bail!("可选工具的恢复记录无效。");
            }
            Ok(Some(r))
        }
        Ok(_) => bail!("可选工具恢复记录过大。"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn states() -> Result<Vec<ToolState>> {
    Ok(vec![ToolState {
        id: LIVELY.id.into(),
        label: LIVELY.label.into(),
        installed: installed(&LIVELY)?,
        enabled: wallpaper_enabled()?,
        managed: record(&LIVELY)?.is_some_and(|s| s.installed_by_winglow),
        supported: true,
        note: "一键优化默认使用 Rain，默认静音；可点击图片手动切换。".into(),
    }])
}
pub fn prepare(id: &str) -> Result<()> {
    let t = tool(id)?;
    let path = root(t)?.join("setup.exe");
    fs::create_dir_all(root(t)?)?;
    if path.exists() {
        checked_bytes(t, &path)?;
        return Ok(());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;
    let response = client.get(t.url).send()?.error_for_status()?;
    let mut bytes = Vec::new();
    response.take(t.size as u64 + 1).read_to_end(&mut bytes)?;
    validate_download(t, &bytes)?;
    transaction::persist_new(&path, &bytes)
}
fn validate_download(t: &Tool, bytes: &[u8]) -> Result<()> {
    if bytes.len() != t.size || format!("{:x}", Sha256::digest(bytes)) != t.hash {
        bail!("{} 官方安装包校验失败，未安装。", t.label);
    }
    Ok(())
}
fn checked_bytes(t: &Tool, path: &Path) -> Result<Vec<u8>> {
    if fs::metadata(path)?.len() != t.size as u64 {
        bail!("安装包大小异常。");
    }
    let bytes = fs::read(path)?;
    validate_download(t, &bytes)?;
    Ok(bytes)
}
#[derive(Serialize, Deserialize)]
struct Backup {
    version: u32,
    files: Vec<(String, Option<Vec<u8>>)>,
    registry: Vec<serde_json::Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallpaperOptimization {
    backup: String,
    installed_before: bool,
    running_before: bool,
}
pub fn prepare_wallpaper_optimization() -> Result<()> {
    WallpaperRefresh::capture()?;
    if !installed(&LIVELY)? {
        prepare("lively")?;
    }
    Ok(())
}
pub fn capture_wallpaper_optimization() -> Result<WallpaperOptimization> {
    Ok(WallpaperOptimization {
        backup: backup(&LIVELY)?.to_string_lossy().into_owned(),
        installed_before: installed(&LIVELY)?,
        running_before: WallpaperRefresh::capture()?.exe.is_some(),
    })
}
fn lively_executable() -> Result<PathBuf> {
    let own = own_exe(&LIVELY)?;
    Ok(if own.is_file() {
        own
    } else {
        lively_existing().context("请先安装动态壁纸。")?
    })
}
fn wallpaper_enabled() -> Result<bool> {
    if !installed(&LIVELY)? || !runtime::own_running(&lively_executable()?)? {
        return Ok(false);
    }
    let layout: serde_json::Value = match fs::read(lively_file("WallpaperLayout.json")?) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    Ok(layout.as_array().is_some_and(|items| !items.is_empty()))
}
pub(crate) fn stop_legacy_rotation() -> Result<()> {
    carousel::stop()
}
pub(crate) fn disable_managed_wallpaper() -> Result<()> {
    carousel::stop()?;
    if installed(&LIVELY)? {
        set_wallpaper_enabled(false)?;
    }
    Ok(())
}
fn wallpaper_disabled_marker() -> Result<PathBuf> {
    Ok(font_engine::data_root()?.join("wallpaper-disabled.json"))
}
fn set_wallpaper_enabled(enabled: bool) -> Result<ActionResult> {
    crate::desktop_access::ensure_compatible()?;
    let marker = wallpaper_disabled_marker()?;
    if enabled {
        if marker.is_file() {
            if fs::metadata(&marker)?.len() > 8192 {
                bail!("壁纸恢复记录无效。");
            }
            let before: WallpaperOptimization = serde_json::from_slice(&fs::read(&marker)?)?;
            restore_wallpaper_optimization(&before)?;
            if !wallpaper_enabled()? {
                apply_wallpaper_optimization()?;
            }
            fs::remove_file(marker)?;
        } else if !wallpaper_enabled()? {
            if !installed(&LIVELY)? {
                prepare("lively")?;
                perform("lively", "install")?;
            }
            apply_wallpaper_optimization()?;
        }
        return Ok(ActionResult {
            message: "动态壁纸已开启。".into(),
        });
    }
    if !wallpaper_enabled()? {
        carousel::stop()?;
        disable_wallpaper_startup()?;
        return Ok(ActionResult {
            message: "动态壁纸已关闭。".into(),
        });
    }
    let before = capture_wallpaper_optimization()?;
    let temp = marker.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    transaction::persist_new(&temp, &serde_json::to_vec(&before)?)?;
    worker::replace_file(&temp, &marker)?;
    let outcome = (|| -> Result<()> {
        carousel::stop()?;
        let exe = lively_executable()?;
        send_wallpaper_command(&exe, &["closewp".into(), "--monitor".into(), "-1".into()])?;
        let deadline = Instant::now() + Duration::from_secs(8);
        while wallpaper_enabled()? {
            if Instant::now() >= deadline {
                bail!("壁纸关闭尚未完成。");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        disable_wallpaper_startup()?;
        Ok(())
    })();
    if let Err(error) = outcome {
        restore_wallpaper_optimization(&before)
            .with_context(|| format!("{error:#}；关闭失败，原壁纸恢复未完成。"))?;
        fs::remove_file(marker)?;
        return Err(error);
    }
    Ok(ActionResult {
        message: "动态壁纸已关闭，开启时恢复之前的壁纸。".into(),
    })
}
fn disable_wallpaper_startup() -> Result<()> {
    use transaction::ValueStore;
    let own = wallpaper_startup_entry()?;
    if crate::registry::WindowsRegistry.read(&own.slot)? == own.value {
        crate::shell_engine::commit(
            "wallpaper:disable-startup",
            vec![crate::registry::Entry {
                slot: own.slot,
                value: None,
            }],
            || Ok(()),
        )?;
    }
    Ok(())
}
pub fn wallpaper_startup_entry() -> Result<crate::registry::Entry> {
    let exe = std::env::current_exe()?;
    Ok(crate::registry::Entry {
        slot: crate::registry::slot(crate::registry::Scope::Startup, "WinGlow-Lively"),
        value: Some(crate::registry::string(&format!(
            "\"{}\" --wallpaper-host",
            exe.display()
        ))),
    })
}
fn scene_project(scene: &str, minutes: u32) -> Result<PathBuf> {
    if !matches!(scene, "carousel" | "night" | "sea" | "dawn")
        || !matches!(minutes, 1 | 5 | 10 | 15 | 30)
    {
        bail!("壁纸轮播参数无效。");
    }
    let settings = lively_file("Settings.json")?;
    let library = if settings.exists() {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(settings)?)?;
        value
            .get("WallpaperDir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
    } else {
        None
    };
    let library = library.unwrap_or(
        lively_file("Settings.json")?
            .parent()
            .unwrap()
            .join("Library"),
    );
    if !library.is_absolute() {
        bail!("壁纸库目录无效，未应用壁纸。");
    }
    let html = scene_html(scene, minutes);
    let fingerprint = format!("{:x}", Sha256::digest(html.as_bytes()));
    let project = library.join(format!(
        "wallpapers/winglow-carousel-{scene}-{minutes}-{}",
        &fingerprint[..12]
    ));
    fs::create_dir_all(&project)?;
    for (name, bytes) in [
        ("index.html", html.as_bytes()),
        (
            "LivelyInfo.json",
            include_bytes!("../wallpaper/LivelyInfo.json").as_slice(),
        ),
    ] {
        let path = project.join(name);
        if path.exists() {
            if fs::read(path)? != bytes {
                bail!("内置壁纸目录已有其他内容，未覆盖。");
            }
        } else {
            transaction::persist_new(&path, bytes)?;
        }
    }
    Ok(project)
}
pub fn apply_wallpaper_optimization() -> Result<()> {
    if !installed(&LIVELY)? {
        perform("lively", "install")?;
    }
    WallpaperRefresh::capture()?.pause()?;
    clear_wallpaper_resume()?;
    new_lively_settings()?;
    let exe = lively_executable()?;
    ensure_lively_started(&exe)?;
    let first = wait_for_first_project()?;
    persist_startup_selection(&first)?;
    carousel::stop()?;
    // A fresh/stopped library may have no monitor entries to rewrite. Starting
    // Lively only restores its saved layout, so explicitly select the default.
    send_wallpaper_command(
        &exe,
        &[
            "setwp".into(),
            "--file".into(),
            first.to_string_lossy().into_owned(),
        ],
    )?;
    wait_for_selection(&exe, &first)
}
fn wait_for_first_project() -> Result<PathBuf> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last_error = None;
    loop {
        match carousel::first_project() {
            Ok(project) => return Ok(project),
            Err(error) => last_error = Some(error),
        }
        if Instant::now() >= deadline {
            let error = last_error.context("默认 Rain 壁纸未出现在壁纸库中。")?;
            bail!("动态壁纸库初始化超时：{error:#}");
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
fn wait_for_selection(exe: &Path, project: &Path) -> Result<()> {
    // Startup restores its saved wallpaper asynchronously. A momentary match
    // can be overwritten by that restore; require a stable selected project.
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut matched_since = None;
    loop {
        if let Ok(bytes) = fs::read(lively_file("WallpaperLayout.json")?)
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
            && layout_contains_project(&value, project)
            && runtime::own_running(exe)?
        {
            let since = matched_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= Duration::from_secs(3) {
                return Ok(());
            }
        } else {
            matched_since = None;
        }
        if Instant::now() >= deadline {
            bail!("动态壁纸尚未完成应用，请在基础美化页重试。");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
fn startup_layout(value: &serde_json::Value, project: &Path) -> Result<serde_json::Value> {
    let mut desired = value.clone();
    let items = desired
        .as_array_mut()
        .context("壁纸布局格式无效，未覆盖。")?;
    if items.len() > 32 {
        bail!("壁纸屏幕数量异常，未覆盖。");
    }
    for item in items {
        let item = item.as_object_mut().context("壁纸屏幕配置无效，未覆盖。")?;
        if !item.contains_key("LivelyScreen") || !item.contains_key("LivelyInfoPath") {
            bail!("壁纸屏幕配置不完整，未覆盖。");
        }
        item.insert(
            "LivelyInfoPath".into(),
            serde_json::json!(project.to_string_lossy()),
        );
    }
    Ok(desired)
}
fn persist_startup_selection(project: &Path) -> Result<()> {
    let path = lively_file("WallpaperLayout.json")?;
    let bytes = match fs::read(&path) {
        Ok(bytes) if bytes.len() <= 1_000_000 => bytes,
        Ok(_) => bail!("壁纸布局文件过大，未覆盖。"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let desired = startup_layout(&serde_json::from_slice(&bytes)?, project)?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    transaction::persist_new(&temporary, &serde_json::to_vec_pretty(&desired)?)?;
    worker::replace_file(&temporary, &path)
}
fn layout_contains_project(value: &serde_json::Value, project: &Path) -> bool {
    value.as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item.get("LivelyInfoPath")
                .and_then(|v| v.as_str())
                .is_some_and(|path| runtime::same_path(Path::new(path), project))
        })
    })
}
pub fn diagnose_carousel() -> Result<()> {
    let _guard = worker::optimization_lock()?;
    crate::desktop_access::ensure_compatible()?;
    prepare_wallpaper_optimization()?;
    let before = capture_wallpaper_optimization()?;
    fs::write(
        font_engine::data_root()?.join("wallpaper-carousel-checkpoint.json"),
        serde_json::to_vec(&before)?,
    )?;
    let applied = apply_wallpaper_optimization();
    let restored = restore_wallpaper_optimization(&before);
    let report = serde_json::json!({
        "selectionAcknowledged": applied.is_ok(),
        "restored": restored.is_ok(),
        "applyError": applied.as_ref().err().map(|e|format!("{e:#}")),
        "restoreError": restored.as_ref().err().map(|e|format!("{e:#}")),
        "backup": before.backup,
        "explorerRestarted": false,
    });
    fs::write(
        font_engine::data_root()?.join("wallpaper-carousel-verification.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    applied?;
    restored?;
    fs::remove_file(font_engine::data_root()?.join("wallpaper-carousel-checkpoint.json"))?;
    Ok(())
}
pub fn recover_carousel_probe() -> Result<()> {
    let _guard = worker::optimization_lock()?;
    let path = font_engine::data_root()?.join("wallpaper-carousel-checkpoint.json");
    if fs::metadata(&path)?.len() > 8192 {
        bail!("壁纸恢复记录过大。");
    }
    let before: WallpaperOptimization = serde_json::from_slice(&fs::read(&path)?)?;
    restore_wallpaper_optimization(&before)?;
    fs::remove_file(path)?;
    Ok(())
}
fn clear_wallpaper_resume() -> Result<()> {
    match fs::remove_file(resume_marker()?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
pub fn restore_wallpaper_optimization(before: &WallpaperOptimization) -> Result<()> {
    carousel::stop()?;
    WallpaperRefresh::capture()?.pause()?;
    clear_wallpaper_resume()?;
    if !before.installed_before && record(&LIVELY)?.is_some() {
        perform("lively", "remove")?;
    }
    restore_profile(&LIVELY, Path::new(&before.backup))?;
    if before.running_before {
        ensure_lively_started(&lively_executable()?)?;
    }
    if carousel::active() {
        carousel::start_host()?;
    }
    Ok(())
}
fn lively_file(name: &str) -> Result<PathBuf> {
    if !matches!(
        name,
        "Settings.json" | "WallpaperLayout.json" | "WinGlowCarousel.json"
    ) {
        bail!("未知壁纸备份文件。");
    }
    Ok(dirs::data_local_dir()
        .context("无法定位壁纸配置。")?
        .join("Lively Wallpaper")
        .join(name))
}
fn backup(t: &Tool) -> Result<PathBuf> {
    let mut b = Backup {
        version: 1,
        files: vec![],
        registry: vec![],
    };
    if t.id == "lively" {
        for name in [
            "Settings.json",
            "WallpaperLayout.json",
            "WinGlowCarousel.json",
        ] {
            let bytes = match fs::read(lively_file(name)?) {
                Ok(b) if b.len() <= 1_000_000 => Some(b),
                Ok(_) => bail!("壁纸配置过大，未修改。"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            };
            b.files.push((name.into(), bytes));
        }
    }
    let dir = font_engine::backup_root()?
        .join("optional-components")
        .join(format!("{}-{}", t.id, uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir)?;
    let bytes = serde_json::to_vec(&b)?;
    if bytes.len() > 2_000_000 {
        bail!("可选工具配置过大，未修改。");
    }
    transaction::persist_new(&dir.join("before.json"), &bytes)?;
    Ok(dir)
}
fn restore_profile(t: &Tool, dir: &Path) -> Result<()> {
    // Only fixed file names / fixed registry namespaces can be restored.
    let base = font_engine::backup_root()?
        .join("optional-components")
        .canonicalize()?;
    let canonical = dir.canonicalize()?;
    if !canonical.starts_with(&base) || canonical.parent() != Some(base.as_path()) {
        bail!("可选工具备份路径不合法。");
    }
    let path = canonical.join("before.json");
    if fs::metadata(&path)?.len() > 2_000_000 {
        bail!("可选工具备份过大。");
    }
    let b: Backup = serde_json::from_slice(&fs::read(path)?)?;
    if b.version != 1 {
        bail!("可选工具备份版本不支持。");
    }
    if t.id == "lively" {
        for (name, value) in b.files {
            let path = lively_file(&name)?;
            if let Some(value) = value {
                fs::create_dir_all(path.parent().unwrap())?;
                let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
                transaction::persist_new(&temp, &value)?;
                worker::replace_file(&temp, &path)?;
            } else if let Err(e) = fs::remove_file(path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(e.into());
            }
        }
    }
    Ok(())
}
fn installer(t: &Tool) -> Result<PathBuf> {
    let path = root(t)?.join("setup.exe");
    checked_bytes(t, &path)?;
    Ok(path)
}
fn new_lively_settings() -> Result<()> {
    let path = lively_file("Settings.json")?;
    let mut settings = if path.exists() {
        serde_json::from_slice::<serde_json::Value>(&fs::read(&path)?)?
    } else {
        serde_json::json!({})
    };
    let obj = settings
        .as_object_mut()
        .context("壁纸配置格式无效，未更改。")?;
    obj.insert("AudioVolumeGlobal".into(), serde_json::json!(0));
    obj.insert("AppFullscreenPause".into(), serde_json::json!(0));
    obj.insert("BatteryPause".into(), serde_json::json!(0));
    obj.insert("Startup".into(), serde_json::json!(false));
    obj.insert("DesktopAutoWallpaper".into(), serde_json::json!(false));
    obj.insert("LockScreenAutoWallpaper".into(), serde_json::json!(false));
    fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    transaction::persist_new(&tmp, &serde_json::to_vec_pretty(&settings)?)?;
    worker::replace_file(&tmp, &path)
}
pub fn perform(id: &str, action: &str) -> Result<ActionResult> {
    let t = tool(id)?;
    match action {
        "enable" => set_wallpaper_enabled(true),
        "disable" => set_wallpaper_enabled(false),
        "install" => {
            if installed(t)? {
                bail!("动态壁纸已安装。");
            }
            if !runtime::processes("Lively.exe")?.is_empty() {
                bail!("已有动态壁纸正在运行，不会覆盖它。");
            }
            let saved = backup(t)?;
            fs::create_dir_all(root(t)?)?;
            let record = Installation {
                version: 1,
                tool_version: t.version.into(),
                backup: saved.to_string_lossy().into_owned(),
                installed_by_winglow: true,
            };
            if !marker(t)?.exists() {
                transaction::persist_new(&marker(t)?, &serde_json::to_vec(&record)?)?;
            }
            new_lively_settings()?;
            let status = runtime::command(&installer(t)?)
                .args([
                    "/VERYSILENT",
                    "/SUPPRESSMSGBOXES",
                    "/NORESTART",
                    "/NOAUTOLAUNCH",
                    "/CURRENTUSER",
                    "/TASKS=",
                ])
                .arg(format!("/DIR={}", root(t)?.join("Application").display()))
                .status()?;
            if !status.success() || !own_exe(t)?.is_file() {
                bail!("动态壁纸安装未完成，恢复记录已保留。");
            }
            Ok(ActionResult {
                message: "动态壁纸已安装，点击壁纸图片即可切换。".into(),
            })
        }
        "open" => {
            let exe = lively_executable()?;
            ensure_lively_started(&exe)?;
            send_wallpaper_command(&exe, &["app".into(), "--showApp".into(), "true".into()])?;
            Ok(ActionResult {
                message: "已打开动态壁纸。".into(),
            })
        }
        "remove" => {
            let record = record(t)?
                .filter(|s| s.installed_by_winglow)
                .context("不会卸载用户原有软件。")?;
            backup(t)?;
            carousel::stop()?;
            WallpaperRefresh::capture()?.pause()?;
            clear_wallpaper_resume()?;
            let uninstall = root(t)?.join("Application/unins000.exe");
            if !uninstall.is_file()
                || !runtime::command(&uninstall)
                    .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
                    .status()?
                    .success()
                || own_exe(t)?.is_file()
            {
                bail!("动态壁纸卸载未完成。");
            }
            restore_profile(t, Path::new(&record.backup))?;
            fs::remove_file(marker(t)?)?;
            Ok(ActionResult {
                message: "已还原动态壁纸，壁纸库与媒体文件保留。".into(),
            })
        }
        _ => bail!("未知可选工具操作。"),
    }
}
pub fn run(id: String, action: String) -> Result<ActionResult> {
    tool(&id)?;
    let _guard = worker::optimization_lock()?;
    crate::shell_engine::ensure_ready()?;
    if action == "install" {
        prepare(&id)?;
    }
    perform(&id, &action)
}

fn media_path(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.to_string_lossy().starts_with("\\\\") || !path.is_file() {
        bail!("请选择本机的视频或动图文件。");
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "mp4" | "webm" | "gif") {
        bail!("支持 MP4、WebM 视频与 GIF 动图。");
    }
    Ok(())
}

fn scene_html(scene: &str, minutes: u32) -> String {
    let style = if scene == "carousel" {
        format!(
            ".scene{{animation-duration:{}s}}.night{{animation-delay:-{}s}}.sea{{animation-delay:-{}s}}.dawn{{animation-delay:-{}s}}",
            minutes * 180,
            minutes * 12,
            minutes * 132,
            minutes * 72
        )
    } else {
        format!(".scene{{animation:none;opacity:0}}.scene.{scene}{{opacity:1}}")
    };
    include_str!("../wallpaper/index.html").replace("</style>", &format!("{style}</style>"))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperCard {
    pub id: String,
    pub title: String,
    pub preview: Option<String>,
    pub scene: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperLibrary {
    pub items: Vec<WallpaperCard>,
    pub selected: Option<String>,
    pub interval_minutes: u32,
    pub volume: u8,
    pub can_restore: bool,
}
fn lively_library() -> Result<PathBuf> {
    let path = lively_file("Settings.json")?;
    let value = if path.exists() {
        let bytes = fs::read(&path)?;
        if bytes.len() > 1_000_000 {
            bail!("壁纸配置过大。");
        }
        serde_json::from_slice::<serde_json::Value>(&bytes)?
    } else {
        serde_json::json!({})
    };
    Ok(value["WallpaperDir"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or(path.parent().unwrap().join("Library")))
}
fn library_project(id: &str) -> Result<PathBuf> {
    let name = id.strip_prefix("library:").context("未知壁纸。")?;
    if name.is_empty() || name.contains(['\\', '/']) || matches!(name, "." | "..") {
        bail!("壁纸标识无效。");
    }
    let root = lively_library()?.join("wallpapers").canonicalize()?;
    let project = root.join(name).canonicalize()?;
    if project.parent() != Some(root.as_path()) || !project.join("LivelyInfo.json").is_file() {
        bail!("壁纸已移除，请刷新列表。");
    }
    let info: serde_json::Value =
        serde_json::from_slice(&fs::read(project.join("LivelyInfo.json"))?)?;
    if !wallpaper_supported(&info) {
        bail!("这个壁纸在当前播放器中出现白屏，暂未接入。原壁纸文件保留，请选择其他壁纸。");
    }
    let file = PathBuf::from(info["FileName"].as_str().unwrap());
    if file.is_absolute() {
        media_path(&file)?;
    } else if !project.join(file).canonicalize()?.starts_with(&project) {
        bail!("壁纸资源不在原项目目录内。");
    }
    Ok(player_path(&project))
}
// Rust canonicalize returns verbatim Windows paths. WebView2's virtual-host
// folder mapping cannot resolve relative assets reliably with that prefix.
fn player_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{unc}"))
    } else {
        PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
    }
}
fn wallpaper_supported(info: &serde_json::Value) -> bool {
    info["FileName"]
        .as_str()
        .is_some_and(|name| !name.is_empty())
}
fn wallpaper_undo_marker() -> Result<PathBuf> {
    Ok(font_engine::data_root()?.join("wallpaper-settings-undo.json"))
}
pub fn wallpaper_library() -> Result<WallpaperLibrary> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut items = vec![];
    let root = lively_library()?.join("wallpapers");
    if let Ok(dirs) = fs::read_dir(&root) {
        let mut dirs = dirs.filter_map(|d| d.ok()).collect::<Vec<_>>();
        dirs.sort_by_key(|d| d.file_name());
        let mut preview_budget = 8_000_000;
        for dir in dirs.into_iter().take(100) {
            let name = dir.file_name().to_string_lossy().into_owned();
            if name.starts_with("winglow-carousel-") {
                continue;
            }
            let id = format!("library:{name}");
            let Ok(project) = library_project(&id) else {
                continue;
            };
            let info = project.join("LivelyInfo.json");
            if fs::metadata(&info)?.len() > 65536 {
                continue;
            }
            let Ok(metadata) = serde_json::from_slice::<serde_json::Value>(&fs::read(info)?) else {
                continue;
            };
            let preview = metadata["Thumbnail"]
                .as_str()
                .filter(|n| !n.contains(['\\', '/']) && !matches!(*n, "." | ".."))
                .and_then(|n| {
                    let p = project.join(n).canonicalize().ok()?;
                    if !p
                        .parent()
                        .is_some_and(|parent| runtime::same_path(parent, &project))
                        || fs::metadata(&p).ok()?.len() > 200_000
                    {
                        return None;
                    }
                    let mime = match p.extension()?.to_str()?.to_ascii_lowercase().as_str() {
                        "jpg" | "jpeg" => "image/jpeg",
                        "png" => "image/png",
                        "webp" => "image/webp",
                        _ => return None,
                    };
                    Some(format!(
                        "data:{mime};base64,{}",
                        STANDARD.encode(fs::read(p).ok()?)
                    ))
                });
            let preview = preview.filter(|v| v.len() <= preview_budget);
            preview_budget -= preview.as_ref().map_or(0, String::len);
            items.push(WallpaperCard {
                id,
                title: metadata["Title"]
                    .as_str()
                    .unwrap_or(&name)
                    .chars()
                    .take(80)
                    .collect(),
                preview,
                scene: None,
            });
        }
    }
    let path = lively_file("Settings.json")?;
    let settings = fs::read(path)
        .ok()
        .and_then(|v| serde_json::from_slice::<serde_json::Value>(&v).ok());
    let volume = settings
        .as_ref()
        .and_then(|v| v["AudioVolumeGlobal"].as_u64())
        .unwrap_or(0)
        .min(100) as u8;
    let layout = fs::read(lively_file("WallpaperLayout.json")?)
        .ok()
        .and_then(|v| serde_json::from_slice::<serde_json::Value>(&v).ok());
    let selected_name = layout
        .and_then(|v| {
            v.as_array()?
                .first()?
                .get("LivelyInfoPath")?
                .as_str()
                .map(PathBuf::from)
        })
        .and_then(|p| p.file_name()?.to_str().map(String::from));
    let mut interval_minutes = 5;
    let selected = selected_name.map(|n| {
        if let Some(tail) = n.strip_prefix("winglow-carousel-") {
            let mut parts = tail.split('-');
            let scene = parts
                .next()
                .filter(|s| matches!(*s, "carousel" | "night" | "sea" | "dawn"))
                .unwrap_or("carousel");
            interval_minutes = parts
                .next()
                .and_then(|s| s.parse().ok())
                .filter(|m| matches!(m, 1 | 5 | 10 | 15 | 30))
                .unwrap_or(5);
            format!("builtin:{scene}")
        } else {
            format!("library:{n}")
        }
    });
    Ok(WallpaperLibrary {
        items,
        selected,
        interval_minutes,
        volume,
        can_restore: wallpaper_undo_marker()?.is_file(),
    })
}
fn wallpaper_change(change: impl FnOnce(&Path) -> Result<()>) -> Result<ActionResult> {
    let _guard = worker::optimization_lock()?;
    crate::shell_engine::ensure_ready()?;
    crate::desktop_access::ensure_compatible()?;
    let exe = lively_executable()?;
    runtime::preflight(&exe)?;
    let before = capture_wallpaper_optimization()?;
    let marker = wallpaper_undo_marker()?;
    let tmp = marker.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    transaction::persist_new(&tmp, &serde_json::to_vec(&before)?)?;
    worker::replace_file(&tmp, &marker)?;
    let outcome = ensure_lively_started(&exe).and_then(|_| change(&exe));
    if let Err(error) = outcome {
        return match restore_wallpaper_optimization(&before) {
            Ok(()) => Err(error),
            Err(restore) => Err(anyhow::anyhow!("{error:#}；原壁纸恢复未完成：{restore:#}")),
        };
    }
    match fs::remove_file(wallpaper_disabled_marker()?) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    Ok(ActionResult {
        message: "壁纸设置已应用。".into(),
    })
}
pub fn select_wallpaper(id: String, minutes: u32) -> Result<ActionResult> {
    if id == "builtin:carousel" {
        bail!("自动轮播已移除，请直接选择一张壁纸。");
    }
    let project = if let Some(scene) = id.strip_prefix("builtin:") {
        scene_project(scene, minutes)?
    } else {
        library_project(&id)?
    };
    wallpaper_change(|exe| {
        carousel::stop()?;
        send_wallpaper_command(
            exe,
            &[
                "setwp".into(),
                "--file".into(),
                project.to_string_lossy().into_owned(),
            ],
        )?;
        wait_for_selection(exe, &project)
    })
}
pub fn set_wallpaper_volume(volume: u8) -> Result<ActionResult> {
    if volume > 100 {
        bail!("音量必须在 0 到 100 之间。");
    }
    wallpaper_change(|exe| {
        // Lively 2.2.1 CLI volume changes only the in-memory settings. Persist
        // while it is stopped, then reload its original wallpaper selection.
        WallpaperRefresh::capture()?.pause()?;
        clear_wallpaper_resume()?;
        let path = lively_file("Settings.json")?;
        let mut settings: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
        settings
            .as_object_mut()
            .context("壁纸配置无效，未覆盖。")?
            .insert("AudioVolumeGlobal".into(), serde_json::json!(volume));
        let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        transaction::persist_new(&tmp, &serde_json::to_vec_pretty(&settings)?)?;
        worker::replace_file(&tmp, &path)?;
        ensure_lively_started(exe)
    })
}
pub fn restore_wallpaper_settings() -> Result<ActionResult> {
    let _guard = worker::optimization_lock()?;
    crate::shell_engine::ensure_ready()?;
    let marker = wallpaper_undo_marker()?;
    if !marker.is_file() {
        bail!("没有可恢复的壁纸修改。");
    }
    if fs::metadata(&marker)?.len() > 8192 {
        bail!("壁纸恢复记录无效。");
    }
    let before: WallpaperOptimization = serde_json::from_slice(&fs::read(&marker)?)?;
    restore_wallpaper_optimization(&before)?;
    fs::remove_file(marker)?;
    Ok(ActionResult {
        message: "已恢复上次修改前的壁纸与音量。".into(),
    })
}

pub fn diagnose_integrated_controls() -> Result<()> {
    let before = capture_wallpaper_optimization()?;
    let previous_undo = fs::read(wallpaper_undo_marker()?).ok();
    let checkpoint = font_engine::data_root()?.join("integrated-controls-checkpoint.json");
    transaction::persist_new(&checkpoint, &serde_json::to_vec(&before)?)?;
    let taskbar_before = crate::taskbar::read_config()?;
    let outcome = (|| {
        select_wallpaper("builtin:night".into(), 5)?;
        if wallpaper_library()?.selected.as_deref() != Some("builtin:night") {
            bail!("夜色选中状态未同步。");
        }
        select_wallpaper("builtin:sea".into(), 5)?;
        if wallpaper_library()?.selected.as_deref() != Some("builtin:sea") {
            bail!("海色选中状态未同步。");
        }
        set_wallpaper_volume(10)?;
        if wallpaper_library()?.volume != 10 {
            bail!("音量状态未同步。");
        }
        restore_wallpaper_settings()?;
        if wallpaper_library()?.volume == 10 {
            bail!("音量没有恢复。");
        }
        if crate::taskbar::read_config()? != taskbar_before {
            bail!("壁纸操作修改了任务栏配置。");
        }
        Ok::<_, anyhow::Error>(())
    })();
    let restored = restore_wallpaper_optimization(&before);
    let report = serde_json::json!({"applyError":outcome.as_ref().err().map(|e|format!("{e:#}")),"restoreError":restored.as_ref().err().map(|e|format!("{e:#}")),"explorerRestarted":false,"classicEnabled":false,"backup":before.backup});
    let path = font_engine::data_root()?.join("integrated-controls-verification.json");
    fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    if restored.is_ok() {
        let marker = wallpaper_undo_marker()?;
        if let Some(bytes) = previous_undo {
            let tmp = marker.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            transaction::persist_new(&tmp, &bytes)?;
            worker::replace_file(&tmp, &marker)?;
        } else if marker.is_file() {
            fs::remove_file(marker)?;
        }
        fs::remove_file(checkpoint)?;
    } else {
        let marker = wallpaper_undo_marker()?;
        let tmp = marker.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        transaction::persist_new(&tmp, &serde_json::to_vec(&before)?)?;
        worker::replace_file(&tmp, &marker)?;
    }
    restored?;
    outcome
}

pub fn wallpaper_control(action: &str, path: Option<PathBuf>) -> Result<ActionResult> {
    if action == "mute" {
        return set_wallpaper_volume(0);
    }
    if action == "restore-settings" {
        return restore_wallpaper_settings();
    }
    if action == "choose" {
        let path = path.context("未选择壁纸文件。")?;
        media_path(&path)?;
        return wallpaper_change(|exe| {
            send_wallpaper_command(
                exe,
                &[
                    "setwp".into(),
                    "--file".into(),
                    path.to_string_lossy().into_owned(),
                ],
            )
        });
    }
    let _guard = worker::optimization_lock()?;
    crate::shell_engine::ensure_ready()?;
    crate::desktop_access::ensure_compatible()?;
    let args: Vec<String> = match action {
        "choose" => {
            let path = path.context("未选择壁纸文件。")?;
            media_path(&path)?;
            vec![
                "setwp".into(),
                "--file".into(),
                path.to_string_lossy().into_owned(),
            ]
        }
        "pause" => vec!["app".into(), "--play".into(), "false".into()],
        "play" => vec!["app".into(), "--play".into(), "true".into()],
        "mute" => vec!["app".into(), "--volume".into(), "0".into()],
        "stop" => vec!["closewp".into(), "--monitor".into(), "-1".into()],
        _ => bail!("未知壁纸操作。"),
    };
    let exe = if own_exe(&LIVELY)?.is_file() {
        own_exe(&LIVELY)?
    } else {
        lively_existing().context("请先安装动态壁纸。")?
    };
    runtime::preflight(&exe)?;
    backup(&LIVELY)?;
    ensure_lively_started(&exe)?;
    send_wallpaper_command(&exe, &args)?;
    Ok(ActionResult {
        message: "已发送壁纸设置，画面由动态壁纸组件更新。".into(),
    })
}
fn ensure_lively_started(exe: &Path) -> Result<()> {
    runtime::preflight(exe)?;
    if !runtime::own_running(exe)? {
        runtime::command(exe).spawn()?;
        let deadline = Instant::now() + Duration::from_secs(8);
        while !runtime::own_running(exe)? {
            if Instant::now() >= deadline {
                bail!("动态壁纸启动超时。");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        // Lively's secondary process forwards commands to the initialized core.
        std::thread::sleep(Duration::from_secs(2));
    }
    Ok(())
}
fn send_wallpaper_command(exe: &Path, args: &[String]) -> Result<()> {
    runtime::preflight(exe)?;
    if !runtime::own_running(exe)? {
        bail!("动态壁纸组件尚未运行。");
    }
    crate::lively_rpc::command(args)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_wallpaper_metadata_is_complete() {
        let metadata: serde_json::Value =
            serde_json::from_slice(include_bytes!("../wallpaper/LivelyInfo.json")).unwrap();
        assert_eq!(metadata["Type"], 1);
        assert_eq!(metadata["FileName"], "index.html");
        assert_eq!(metadata["IsAbsolutePath"], false);
    }
    #[test]
    fn startup_selection_replaces_old_wallpapers_without_changing_monitor_layout() {
        let before = serde_json::json!([
            {"LivelyScreen":{"Index":1,"Bounds":"0,0,3840,2160"},"LivelyInfoPath":"old1","custom":true},
            {"LivelyScreen":{"Index":2,"Bounds":"-2560,0,2560,1440"},"LivelyInfoPath":"old2"}
        ]);
        let project = Path::new("C:\\wallpaper\\carousel");
        let desired = startup_layout(&before, project).unwrap();
        assert_eq!(before[0]["LivelyInfoPath"], "old1");
        assert_eq!(desired[0]["LivelyScreen"], before[0]["LivelyScreen"]);
        assert_eq!(desired[1]["LivelyScreen"], before[1]["LivelyScreen"]);
        assert_eq!(desired[0]["custom"], true);
        assert_eq!(
            desired[1]["LivelyInfoPath"],
            project.to_string_lossy().as_ref()
        );
        assert!(startup_layout(&serde_json::json!([{}]), project).is_err());
        assert!(startup_layout(&serde_json::json!({}), project).is_err());
    }
    #[test]
    fn scene_selection_and_rotation_do_not_require_remote_scripts() {
        let scene = scene_html("sea", 5);
        assert!(scene.contains(".scene.sea{opacity:1}"));
        assert!(scene.contains(".scene{animation:none;opacity:0}"));
        let carousel = scene_html("carousel", 10);
        assert!(carousel.contains("animation-duration:1800s"));
        assert!(carousel.contains("animation-delay:-1320s"));
        for id in ["library:..", "library:a/b", "library:a\\b", "file:C:\\x"] {
            assert!(library_project(id).is_err());
        }
        assert!(set_wallpaper_volume(101).is_err());
    }

    #[test]
    fn wallpaper_ack_requires_the_actual_selected_project() {
        let project = std::env::temp_dir().join("winglow-carousel-v1");
        let selected = serde_json::json!([{"LivelyInfoPath":project.to_string_lossy()}]);
        assert!(layout_contains_project(&selected, &project));
        assert!(!layout_contains_project(&serde_json::json!([]), &project));
        assert!(!layout_contains_project(
            &serde_json::json!([{"LivelyInfoPath":"other"}]),
            &project
        ));
    }
    #[test]
    fn wallpaper_requires_existing_local_media() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&dir).unwrap();
        let video = dir.join("壁纸.mp4");
        let executable = dir.join("script.exe");
        fs::write(&video, b"test").unwrap();
        fs::write(&executable, b"test").unwrap();
        assert!(media_path(&video).is_ok());
        assert!(media_path(&executable).is_err());
        assert!(media_path(Path::new("relative.mp4")).is_err());
        assert!(media_path(&dir.join("missing.mp4")).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn closed_tool_and_action_paths() {
        assert!(tool("../../setup.exe").is_err());
        assert!(lively_file("../Library").is_err());
    }

    #[test]
    fn player_paths_keep_unicode_and_unc_without_verbatim_prefix() {
        assert_eq!(
            player_path(Path::new(r"\\?\C:\壁纸\Rain")),
            Path::new(r"C:\壁纸\Rain")
        );
        assert_eq!(
            player_path(Path::new(r"\\?\UNC\server\share\壁纸")),
            Path::new(r"\\server\share\壁纸")
        );
        assert_eq!(
            player_path(Path::new(r"C:\wallpaper")),
            Path::new(r"C:\wallpaper")
        );
        assert!(tool("explorer-patcher").is_err());
    }
}
