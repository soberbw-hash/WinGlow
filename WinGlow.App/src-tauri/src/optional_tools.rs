//! Opt-in upstream applications, installed only when the user selects Install.
//! Keep external installations separate: never uninstall an instance WinGlow didn't install.
use crate::{component_runtime as runtime, font_engine, models::ActionResult, transaction, worker};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::windows::ffi::OsStrExt;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use winreg::{RegKey, RegValue, enums::*};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolState {
    pub id: String,
    pub label: String,
    pub installed: bool,
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
const EP: Tool = Tool {
    id: "explorer-patcher",
    label: "经典布局",
    version: "26100.8457.70.3",
    url: "https://github.com/valinet/ExplorerPatcher/releases/download/26100.8457.70.3/ep_setup.exe",
    hash: "8146db4d3a87201fb80ad1d3712ba8f56883e9a0811758bd39f62d73a9f2c586",
    size: 12237312,
};
fn tool(id: &str) -> Result<&'static Tool> {
    match id {
        "lively" => Ok(&LIVELY),
        "explorer-patcher" => Ok(&EP),
        _ => bail!("未知可选工具。"),
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
fn program_files() -> Result<PathBuf> {
    use windows::Win32::UI::Shell::{FOLDERID_ProgramFiles, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    let value = unsafe { SHGetKnownFolderPath(&FOLDERID_ProgramFiles, KF_FLAG_DEFAULT, None) }?;
    let path = unsafe { value.to_string() }?;
    unsafe {
        windows::Win32::System::Com::CoTaskMemFree(Some(value.0 as *const _));
    }
    Ok(PathBuf::from(path))
}
fn own_exe(t: &Tool) -> Result<PathBuf> {
    Ok(if t.id == "lively" {
        root(t)?.join("Application/Lively.exe")
    } else {
        program_files()?.join("ExplorerPatcher/ep_gui.dll")
    })
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
        // Upstream's secondary process sends its command asynchronously. A zero
        // exit code alone is not acknowledgement; retry while the original lives.
        for _ in 0..3 {
            if !runtime::own_running(exe)? {
                return Ok(());
            }
            let mut request = runtime::command(exe)
                .args(["app", "--shutdown", "true"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(status) = request.try_wait()? {
                    if !status.success() {
                        bail!("动态壁纸未正常响应退出请求，未刷新桌面。");
                    }
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = request.kill(); // Only the newly spawned command helper.
                    bail!("动态壁纸退出请求超时，未刷新桌面。");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            while runtime::own_running(exe)? {
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
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
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
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
    fs::remove_file(marker)?;
    Ok(())
}
fn installed(t: &Tool) -> Result<bool> {
    Ok(own_exe(t)?.is_file() || t.id == "lively" && lively_existing().is_some())
}
fn ep_supported() -> bool {
    let b = crate::visual::build();
    cfg!(target_arch = "x86_64") && ep_build_supported(b)
}
fn ep_build_supported(build: u32) -> bool {
    matches!(build, 22621 | 22631 | 26100 | 26200)
}
pub fn ensure_classic_settings_supported() -> Result<()> {
    if !ep_supported() || sac_enabled() {
        bail!("当前系统未通过经典布局兼容性检查，已阻止启用和设置；仍可恢复系统任务栏。");
    }
    if !installed(&EP)? {
        bail!("请先安装经典布局。");
    }
    Ok(())
}
fn sac_enabled() -> bool {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SYSTEM\\CurrentControlSet\\Control\\CI\\Policy")
        .ok()
        .and_then(|k| {
            k.get_value::<u32, _>("VerifiedAndReputablePolicyState")
                .ok()
        })
        .is_some_and(|v| v == 1)
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
    [&LIVELY, &EP]
        .iter()
        .map(|t| {
            Ok(ToolState {
                id: t.id.into(),
                label: t.label.into(),
                installed: installed(t)?,
                managed: record(t)?.is_some_and(|s| s.installed_by_winglow),
                supported: t.id == "lively" || ep_supported() && !sac_enabled(),
                note: if t.id == "lively" {
                    "一键优化自动启用精选壁纸轮播；静音，全屏和电池模式暂停。也可更换自己的视频。"
                        .into()
                } else if sac_enabled() {
                    "智能应用控制已开启，暂不安装经典布局。".into()
                } else if !ep_supported() {
                    "当前 Windows 版本尚未验证经典布局。".into()
                } else {
                    "ExplorerPatcher：可调整任务栏和开始菜单；不会随一键优化安装。".into()
                },
            })
        })
        .collect()
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
struct RegistryBackup {
    machine: bool,
    tree: Option<RegistryTree>,
}
#[derive(Serialize, Deserialize)]
struct RegistryTree {
    values: Vec<(String, u32, Vec<u8>)>,
    children: Vec<(String, RegistryTree)>,
}
fn capture_tree(key: &RegKey, depth: usize) -> Result<RegistryTree> {
    if depth > 16 {
        bail!("经典布局配置层级过深，未修改。");
    }
    let values = key
        .enum_values()
        .map(|v| v.map(|(name, v)| (name, v.vtype as u32, v.bytes.to_vec())))
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut children = Vec::new();
    for name in key.enum_keys() {
        let name = name?;
        children.push((
            name.clone(),
            capture_tree(&key.open_subkey(name)?, depth + 1)?,
        ));
    }
    Ok(RegistryTree { values, children })
}
fn validate_tree(tree: &RegistryTree, depth: usize) -> Result<()> {
    if depth > 16 {
        bail!("经典布局备份层级无效。");
    }
    for (name, kind, _) in &tree.values {
        if name.contains('\0') || !matches!(kind, 0 | 1 | 2 | 3 | 4 | 7 | 11) {
            bail!("经典布局备份值无效。");
        }
    }
    for (name, child) in &tree.children {
        if name.is_empty() || name.contains(['\\', '/', '\0']) {
            bail!("经典布局备份键无效。");
        }
        validate_tree(child, depth + 1)?;
    }
    Ok(())
}
fn restore_tree(key: &RegKey, tree: RegistryTree) -> Result<()> {
    for (name, kind, bytes) in tree.values {
        let vtype = match kind {
            0 => REG_NONE,
            1 => REG_SZ,
            2 => REG_EXPAND_SZ,
            3 => REG_BINARY,
            4 => REG_DWORD,
            7 => REG_MULTI_SZ,
            11 => REG_QWORD,
            _ => bail!("配置备份类型无效。"),
        };
        key.set_raw_value(
            name,
            &RegValue {
                bytes: bytes.into(),
                vtype,
            },
        )?;
    }
    for (name, child) in tree.children {
        restore_tree(&key.create_subkey(name)?.0, child)?;
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct Backup {
    version: u32,
    files: Vec<(String, Option<Vec<u8>>)>,
    registry: Vec<RegistryBackup>,
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
pub fn wallpaper_startup_entry() -> Result<crate::registry::Entry> {
    let exe = lively_existing().unwrap_or(own_exe(&LIVELY)?);
    Ok(crate::registry::Entry {
        slot: crate::registry::slot(crate::registry::Scope::Startup, "WinGlow-Lively"),
        value: Some(crate::registry::string(&format!("\"{}\"", exe.display()))),
    })
}
fn carousel_project() -> Result<PathBuf> {
    scene_project("carousel", 5)
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
    let project = carousel_project()?;
    let exe = lively_executable()?;
    ensure_lively_started(&exe)?;
    send_wallpaper_command(
        &exe,
        &[
            "setwp".into(),
            "--file".into(),
            project.to_string_lossy().into_owned(),
        ],
    )?;
    // CLI exit only acknowledges dispatch. Require Lively's saved selection too.
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        if let Ok(bytes) = fs::read(lively_file("WallpaperLayout.json")?)
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
            && layout_contains_project(&value, &project)
            && runtime::own_running(&exe)?
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("动态壁纸尚未完成应用，请查看壁纸库；本次优化会恢复原设置。");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
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
    WallpaperRefresh::capture()?.pause()?;
    clear_wallpaper_resume()?;
    if !before.installed_before && record(&LIVELY)?.is_some() {
        perform("lively", "remove")?;
    }
    restore_profile(&LIVELY, Path::new(&before.backup))?;
    if before.running_before {
        ensure_lively_started(&lively_executable()?)?;
    }
    Ok(())
}
fn lively_file(name: &str) -> Result<PathBuf> {
    if !matches!(name, "Settings.json" | "WallpaperLayout.json") {
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
        for name in ["Settings.json", "WallpaperLayout.json"] {
            let bytes = match fs::read(lively_file(name)?) {
                Ok(b) if b.len() <= 1_000_000 => Some(b),
                Ok(_) => bail!("壁纸配置过大，未修改。"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            };
            b.files.push((name.into(), bytes));
        }
    } else {
        for machine in [false, true] {
            let tree = match RegKey::predef(if machine {
                HKEY_LOCAL_MACHINE
            } else {
                HKEY_CURRENT_USER
            })
            .open_subkey("Software\\ExplorerPatcher")
            {
                Ok(k) => Some(capture_tree(&k, 0)?),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            };
            b.registry.push(RegistryBackup { machine, tree });
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
    } else {
        for saved in &b.registry {
            if let Some(tree) = &saved.tree {
                validate_tree(tree, 0)?;
            }
        }
        for saved in b.registry {
            let hive = RegKey::predef(if saved.machine {
                HKEY_LOCAL_MACHINE
            } else {
                HKEY_CURRENT_USER
            });
            match hive.delete_subkey_all("Software\\ExplorerPatcher") {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
            if let Some(tree) = saved.tree {
                restore_tree(&hive.create_subkey("Software\\ExplorerPatcher")?.0, tree)?;
            }
        }
    }
    Ok(())
}
fn installer(t: &Tool) -> Result<PathBuf> {
    let cache = root(t)?.join("setup.exe");
    let bytes = checked_bytes(t, &cache)?;
    if t.id == "lively" {
        return Ok(cache);
    }
    // Elevated EP execution uses an administrator-owned directory, never a user-writable cache.
    let dest = program_files()?
        .join("WinGlow Components/ExplorerPatcher")
        .join(t.version)
        .join("ep_setup.exe");
    fs::create_dir_all(dest.parent().unwrap())?;
    if dest.exists() {
        checked_bytes(t, &dest)?;
    } else {
        transaction::persist_new(&dest, &bytes)?;
    }
    Ok(dest)
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
        "install" => {
            if installed(t)? {
                bail!("该工具已安装，请使用打开入口。不会覆盖已有安装。");
            }
            if t.id == "explorer-patcher" && (!ep_supported() || sac_enabled()) {
                bail!("当前系统不适合安装经典布局。未修改智能应用控制或安全软件设置。");
            }
            if t.id == "lively" && !runtime::processes("Lively.exe")?.is_empty() {
                bail!("已有动态壁纸正在运行，不会覆盖它。");
            }
            let dir = backup(t)?;
            let mut r = record(t)?.unwrap_or(Installation {
                version: 1,
                tool_version: t.version.into(),
                backup: dir.to_string_lossy().into_owned(),
                installed_by_winglow: true,
            });
            r.installed_by_winglow = true;
            fs::create_dir_all(root(t)?)?;
            if !marker(t)?.exists() {
                transaction::persist_new(&marker(t)?, &serde_json::to_vec(&r)?)?;
            }
            let setup = installer(t)?;
            let result = (|| {
                let mut cmd = runtime::command(&setup);
                if t.id == "lively" {
                    new_lively_settings()?;
                    cmd.args([
                        "/VERYSILENT",
                        "/SUPPRESSMSGBOXES",
                        "/NORESTART",
                        "/NOAUTOLAUNCH",
                        "/CURRENTUSER",
                        "/TASKS=",
                    ])
                    .arg(format!("/DIR={}", root(t)?.join("Application").display()));
                } else {
                    cmd.arg("/update_silent");
                }
                let status = cmd.status()?;
                if !status.success() || !own_exe(t)?.is_file() {
                    bail!("{} 安装未完成。", t.label);
                }
                Ok(())
            })();
            if let Err(e) = result {
                // Keep the ownership/checkpoint record for a partial external installation.
                if !own_exe(t)?.is_file() {
                    restore_profile(t, Path::new(&r.backup))?;
                    fs::remove_file(marker(t)?)?;
                }
                return Err(e);
            }
            Ok(ActionResult {
                message: format!("{} 已安装。使用该功能右侧的设置按钮。", t.label),
            })
        }
        "open" => {
            if t.id == "explorer-patcher" {
                ensure_classic_settings_supported()?;
            }
            if !installed(t)? {
                bail!("请先安装该可选功能。");
            }
            backup(t)?;
            if t.id == "lively" {
                let exe = if own_exe(t)?.exists() {
                    own_exe(t)?
                } else {
                    lively_existing().context("无法定位已有壁纸软件。")?
                };
                runtime::preflight(&exe)?;
                if runtime::own_running(&exe)? {
                    runtime::command(&exe)
                        .args(["app", "--showApp", "true"])
                        .spawn()?;
                } else {
                    runtime::command(&exe).spawn()?;
                }
            } else {
                let arg = format!("\"{}\",ZZGUI", own_exe(t)?.display());
                use windows::{
                    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
                    core::PCWSTR,
                };
                let file: Vec<u16> = runtime::system_exe("rundll32.exe")?
                    .as_os_str()
                    .encode_wide()
                    .chain(Some(0))
                    .collect();
                let params: Vec<u16> = arg.encode_utf16().chain(Some(0)).collect();
                let result = unsafe {
                    ShellExecuteW(
                        None,
                        PCWSTR::null(),
                        PCWSTR(file.as_ptr()),
                        PCWSTR(params.as_ptr()),
                        PCWSTR::null(),
                        SW_SHOWNORMAL,
                    )
                };
                if result.0 as usize <= 32 {
                    bail!("无法打开经典布局设置。");
                }
            }
            Ok(ActionResult {
                message: format!("已打开{}。", t.label),
            })
        }
        "system-taskbar" if t.id == "explorer-patcher" => {
            if !installed(t)? {
                bail!("经典布局未安装，无需恢复。");
            }
            backup(t)?;
            let (key, _) =
                RegKey::predef(HKEY_CURRENT_USER).create_subkey("Software\\ExplorerPatcher")?;
            key.set_value("OldTaskbar", &0u32)?;
            Ok(ActionResult {
                message: "已恢复系统任务栏，经典布局的其他设置保留。".into(),
            })
        }
        "remove" => {
            let r = record(t)?
                .filter(|s| s.installed_by_winglow)
                .context("该工具不是由 WinGlow 安装，不会卸载用户原有软件。")?;
            backup(t)?;
            if t.id == "lively" {
                let exe = own_exe(t)?;
                runtime::preflight(&exe)?;
                if runtime::own_running(&exe)? {
                    runtime::command(&exe)
                        .args(["app", "--shutdown", "true"])
                        .status()?;
                    let until = Instant::now() + Duration::from_secs(8);
                    while runtime::own_running(&exe)? && Instant::now() < until {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    if runtime::own_running(&exe)? {
                        bail!("动态壁纸尚未退出，未强制关闭。");
                    }
                }
                let uninstaller = root(t)?.join("Application/unins000.exe");
                if uninstaller.exists()
                    && !runtime::command(&uninstaller)
                        .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
                        .status()?
                        .success()
                {
                    bail!("动态壁纸卸载未完成。");
                }
            } else if own_exe(t)?.is_file()
                && !runtime::command(&installer(t)?)
                    .arg("/uninstall_silent")
                    .status()?
                    .success()
            {
                bail!("经典布局卸载未完成。");
            }
            if own_exe(t)?.is_file() {
                bail!("可选工具仍在安装目录中，请重试还原。");
            }
            restore_profile(t, Path::new(&r.backup))?;
            fs::remove_file(marker(t)?)?;
            Ok(ActionResult {
                message: format!("已还原{}；保留你的壁纸库和媒体文件。", t.label),
            })
        }
        _ => bail!("未知可选工具操作。"),
    }
}
pub fn run(id: String, action: String) -> Result<ActionResult> {
    tool(&id)?;
    let _guard = worker::optimization_lock()?;
    crate::shell_engine::ensure_ready()?;
    if id == "explorer-patcher" && action == "install" && (!ep_supported() || sac_enabled()) {
        bail!("当前系统未通过经典布局兼容验证，未安装。其他美化功能可正常使用。");
    }
    if id == "explorer-patcher" && action == "system-taskbar" {
        let result = worker::run_inner(crate::models::Operation::OptionalTool { id, verb: action });
        return crate::explorer::finish(result, true, crate::explorer::restart);
    }
    if action == "install"
        || action == "remove" && id == "explorer-patcher" && own_exe(tool(&id)?)?.is_file()
    {
        prepare(&id)?;
    }
    // EP's installer refreshes Explorer itself; protect all desktop-dependent effects.
    let refresh = id == "explorer-patcher" && matches!(action.as_str(), "install" | "remove");
    let operation =
        || worker::run_inner(crate::models::Operation::OptionalTool { id, verb: action });
    if refresh {
        crate::explorer::external_refresh(operation)
    } else {
        operation()
    }
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
    Ok(project)
}
fn wallpaper_supported(info: &serde_json::Value) -> bool {
    // Observed on the bundled Parallax demo: unstyled page and broken images.
    // Do not offer a known-broken project until its player compatibility is verified.
    !info["Title"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains("parallax")
}
fn wallpaper_undo_marker() -> Result<PathBuf> {
    Ok(font_engine::data_root()?.join("wallpaper-settings-undo.json"))
}
pub fn wallpaper_library() -> Result<WallpaperLibrary> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut items = vec![];
    for (id, title, scene) in [
        ("carousel", "自动轮播", None),
        ("night", "夜色", Some("night")),
        ("sea", "海色", Some("sea")),
        ("dawn", "晨光", Some("dawn")),
    ] {
        items.push(WallpaperCard {
            id: format!("builtin:{id}"),
            title: title.into(),
            preview: None,
            scene: scene.map(String::from),
        });
    }
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
                    if p.parent() != Some(project.as_path())
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
    Ok(ActionResult {
        message: "壁纸设置已应用。".into(),
    })
}
pub fn select_wallpaper(id: String, minutes: u32) -> Result<ActionResult> {
    let project = if let Some(scene) = id.strip_prefix("builtin:") {
        scene_project(scene, minutes)?
    } else {
        library_project(&id)?
    };
    wallpaper_change(|exe| {
        send_wallpaper_command(
            exe,
            &[
                "setwp".into(),
                "--file".into(),
                project.to_string_lossy().into_owned(),
            ],
        )?;
        let deadline = Instant::now() + Duration::from_secs(25);
        loop {
            if fs::read(lively_file("WallpaperLayout.json")?)
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .is_some_and(|v| layout_contains_project(&v, &project))
            {
                break;
            }
            if Instant::now() >= deadline {
                bail!("壁纸尚未完成加载，已尝试恢复原设置。");
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Ok(())
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
        if !ep_supported() {
            let settings = crate::desktop_settings::classic_settings()?;
            if crate::desktop_settings::apply_classic(settings).is_ok() {
                bail!("不兼容系统没有拦截经典布局设置。");
            }
        }
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
    let mut child = runtime::command(exe).args(args).spawn()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                bail!("动态壁纸未接受操作请求。");
            }
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("动态壁纸操作超时，请打开壁纸库检查组件状态。");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
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
    fn reject_untested_explorer_patcher_builds() {
        for build in [22621, 22631, 26100, 26200] {
            assert!(ep_build_supported(build));
        }
        for build in [22000, 26300, 28000, 30000] {
            assert!(!ep_build_supported(build));
        }
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
    fn reject_wrong_download_before_install() {
        assert!(validate_download(&EP, b"MZ").is_err());
    }
    #[test]
    fn registry_snapshot_roundtrip_preserves_nested_raw_values() {
        let hive = RegKey::predef(HKEY_CURRENT_USER);
        let path = format!("Software\\WinGlowTests\\{}", uuid::Uuid::new_v4());
        let (key, _) = hive.create_subkey(&path).unwrap();
        let result = (|| -> Result<()> {
            key.set_value("title", &"original")?;
            key.create_subkey("nested")?.0.set_raw_value(
                "bytes",
                &RegValue {
                    vtype: REG_BINARY,
                    bytes: vec![0, 255, 42].into(),
                },
            )?;
            let before = capture_tree(&key, 0)?;
            validate_tree(&before, 0)?;
            let encoded = serde_json::to_vec(&before)?;
            drop(key);
            hive.delete_subkey_all(&path)?;
            restore_tree(
                &hive.create_subkey(&path)?.0,
                serde_json::from_slice(&encoded)?,
            )?;
            let after = capture_tree(&hive.open_subkey(&path)?, 0)?;
            assert_eq!(serde_json::to_vec(&after)?, encoded);
            Ok(())
        })();
        hive.delete_subkey_all(&path).unwrap();
        result.unwrap();
    }
    #[test]
    fn reject_registry_snapshot_path_escape_before_restoring() {
        let tree = RegistryTree {
            values: vec![],
            children: vec![(
                "../other".into(),
                RegistryTree {
                    values: vec![],
                    children: vec![],
                },
            )],
        };
        assert!(validate_tree(&tree, 0).is_err());
    }
}
