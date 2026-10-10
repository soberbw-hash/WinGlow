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
    cfg!(target_arch = "x86_64") && matches!(b, 22621 | 22631 | 26100 | 26200 | 26300 | 28000)
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
                    "选择动态壁纸；默认静音，全屏和电池模式暂停。".into()
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
                message: format!("{} 已安装。点击打开，选择你喜欢的效果。", t.label),
            })
        }
        "open" => {
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
                runtime::command(&exe)
                    .args(["app", "--showApp", "true"])
                    .spawn()?;
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
    if action == "install"
        || action == "remove" && id == "explorer-patcher" && own_exe(tool(&id)?)?.is_file()
    {
        prepare(&id)?;
    }
    // EP's installer refreshes Explorer itself. Pause TTB before it, and always restore runtime.
    let refresh = id == "explorer-patcher" && matches!(action.as_str(), "install" | "remove");
    if refresh {
        crate::taskbar::preflight()?;
        crate::taskbar::pause_owned()?;
    }
    let result = worker::run_inner(crate::models::Operation::OptionalTool { id, verb: action });
    let resume = if refresh {
        crate::explorer::wait_for_taskbar().and_then(|_| crate::taskbar::sync())
    } else {
        Ok(())
    };
    match (result, resume) {
        (Ok(mut result), Err(e)) => {
            result
                .message
                .push_str(&format!(" 任务栏恢复未完成：{e:#}"));
            Ok(result)
        }
        (Err(e), Err(resume)) => Err(e.context(format!("任务栏恢复未完成：{resume:#}"))),
        (result, _) => result,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
