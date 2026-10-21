//! Separate, unmodified upstream executable. Download only on explicit enable.
use crate::{
    font_engine,
    models::ActionResult,
    registry::{self, Entry, Scope, WindowsRegistry},
    shell_engine,
    transaction::ValueStore,
};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::CloseHandle,
        System::Threading::{EVENT_MODIFY_STATE, OpenEventW, SetEvent},
    },
    core::PCWSTR,
};
const URL: &str =
    "https://github.com/std-microblock/breeze-shell/releases/download/0.1.34/windows-build.zip";
const HASH: &str = "80294ffea75113a7f65eecbe777b579ce671559f0b49f2a83b291ec865f433c2";
const CREATE_NO_WINDOW: u32 = 0x08000000;
fn root() -> Result<PathBuf> {
    Ok(font_engine::data_root()?.join("Breeze").join("0.1.34"))
}
fn exe() -> Result<PathBuf> {
    Ok(root()?.join("breeze.exe"))
}
pub fn open_config_window() -> Result<()> {
    if !running()? {
        bail!("请先在“右键菜单”页面开启 Breeze 美化，再打开设置菜单。")
    }
    ensure_owned_runtime()?;
    let data_dir = PathBuf::from(std::env::var_os("USERPROFILE").context("无法定位当前用户目录")?)
        .join(".breeze-shell");
    let id = uuid::Uuid::new_v4().to_string();
    let request = data_dir.join(format!("WinGlow-Open-Config-{id}.request"));
    let completed = data_dir.join(format!("WinGlow-Open-Config-{id}.request.done"));
    fs::write(&request, &id).context("无法通知 Breeze 打开设置菜单")?;
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if fs::read_to_string(&completed).is_ok_and(|value| value == id) {
            let _ = fs::remove_file(&completed);
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = fs::remove_file(&request);
    let _ = fs::remove_file(&completed);
    bail!("Breeze 没有响应。请确认右键菜单页的 Breeze 美化已开启后重试。")
}
fn slot() -> registry::Slot {
    registry::slot(Scope::Startup, "WinGlow-Breeze")
}
fn legacy_slot() -> registry::Slot {
    registry::slot(Scope::Startup, "WindowsWeitiao-Breeze")
}
fn command() -> Result<String> {
    let windir = std::env::var_os("WINDIR").context("无法定位 Windows 系统目录")?;
    let launcher = PathBuf::from(windir).join("System32").join("wscript.exe");
    let script = root()?.join("start-hidden.vbs");
    Ok(format!(
        "\"{}\" //B //NoLogo \"{}\"",
        launcher.display(),
        script.display()
    ))
}
fn direct_command() -> Result<String> {
    Ok(format!("\"{}\" inject-consistent", exe()?.display()))
}
fn is_breeze_startup_command(value: &str, hidden_launcher: &str, old_direct: &str) -> bool {
    value == hidden_launcher || value == old_direct
}
fn needs_startup_migration(value: &str, old_direct: &str) -> bool {
    value == old_direct
}
fn hidden_launcher_script(executable: &std::path::Path) -> String {
    let command = format!("\"{}\" inject-consistent", executable.display());
    let vbs_command = command.replace('"', "\"\"");
    format!("CreateObject(\"WScript.Shell\").Run \"{vbs_command}\", 0, False\r\n")
}
fn ensure_hidden_launcher() -> Result<PathBuf> {
    let executable = exe()?;
    if !executable.is_file() {
        bail!("找不到 Breeze 启动文件，无法配置登录启动。");
    }
    let script = root()?.join("start-hidden.vbs");
    let contents = hidden_launcher_script(&executable);
    if fs::read_to_string(&script).ok().as_deref() != Some(&contents) {
        fs::write(&script, contents).context("无法写入 Breeze 隐藏启动脚本")?;
    }
    Ok(script)
}
pub fn enabled() -> Result<bool> {
    let expected = command()?;
    let old_direct = direct_command()?;
    for candidate in [slot(), legacy_slot()] {
        if WindowsRegistry
            .read(&candidate)?
            .and_then(|v| registry::as_string(&v))
            .is_some_and(|s| is_breeze_startup_command(&s, &expected, &old_direct))
        {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn install() -> Result<PathBuf> {
    let root = root()?;
    fs::create_dir_all(&root)?;
    let archive = root.join("upstream.zip");
    let bytes = if archive.is_file() {
        fs::read(&archive)?
    } else {
        let response = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?
            .get(URL)
            .send()?
            .error_for_status()?;
        let mut bytes = Vec::new();
        response.take(8_000_001).read_to_end(&mut bytes)?;
        if bytes.len() > 8_000_000 {
            bail!("Breeze 下载超出预期大小。");
        }
        if format!("{:x}", Sha256::digest(&bytes)) != HASH {
            bail!("Breeze 下载校验失败，未启动。");
        }
        crate::transaction::persist_new(&archive, &bytes)?;
        bytes
    };
    if format!("{:x}", Sha256::digest(&bytes)) != HASH {
        bail!("Breeze 文件校验失败，未启动。");
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
    // Extract only the two reviewed filenames, never arbitrary ZIP paths.
    for name in ["breeze.exe", "shell.dll"] {
        let mut entry = zip.by_name(&format!("x64/releasedbg/{name}"))?;
        if entry.size() > 16_000_000 {
            bail!("Breeze 文件大小异常。");
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        let path = root.join(name);
        if path.exists() {
            if fs::read(&path)? != data {
                bail!("Breeze 资源被修改，未启动。");
            }
        } else {
            crate::transaction::persist_new(&path, &data)?;
        }
    }
    ensure_hidden_launcher()?;
    fs::write(
        root.join("LICENSE.txt"),
        include_str!("../../../licenses/Breeze-AGPL-3.0.txt"),
    )?;
    fs::write(
        root.join("SOURCE.txt"),
        "Breeze Shell 0.1.34, unmodified upstream runtime\nhttps://github.com/std-microblock/breeze-shell/tree/0.1.34\nhttps://github.com/std-microblock/breeze-shell/releases/tag/0.1.34\nAGPL-3.0. Downloaded from upstream by this user; not bundled with WinGlow.\n",
    )?;
    Ok(root.join("breeze.exe"))
}
pub fn stop() -> Result<()> {
    if !running()? {
        return Ok(());
    }
    ensure_owned_runtime()?;
    let name: Vec<_> = "breeze-shell-inject-consistent-exit"
        .encode_utf16()
        .chain([0])
        .collect();
    match unsafe { OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr())) } {
        Ok(handle) => {
            let outcome = unsafe { SetEvent(handle) };
            unsafe {
                let _ = CloseHandle(handle);
            }
            outcome?;
        }
        Err(error) if error.code().0 as u32 == 0x80070002 => {}
        Err(error) => return Err(error.into()),
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while running()? {
        if Instant::now() >= deadline {
            bail!("Breeze 尚未退出，请稍后重试；没有强制结束其他程序。");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
pub fn startup_entry() -> Result<Entry> {
    ensure_hidden_launcher()?;
    Ok(Entry {
        slot: slot(),
        value: Some(registry::string(&command()?)),
    })
}
pub fn startup_entries() -> Result<Vec<Entry>> {
    let mut entries = vec![startup_entry()?];
    let expected = command()?;
    let old_direct = direct_command()?;
    if WindowsRegistry
        .read(&legacy_slot())?
        .and_then(|v| registry::as_string(&v))
        .is_some_and(|s| is_breeze_startup_command(&s, &expected, &old_direct))
    {
        entries.push(Entry {
            slot: legacy_slot(),
            value: None,
        });
    }
    Ok(entries)
}
pub fn running() -> Result<bool> {
    let name: Vec<_> = "breeze-shell-inject-consistent-exit"
        .encode_utf16()
        .chain([0])
        .collect();
    match unsafe { OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr())) } {
        Ok(handle) => {
            unsafe {
                let _ = CloseHandle(handle);
            }
            Ok(true)
        }
        Err(error) if error.code().0 as u32 == 0x80070002 => Ok(false),
        Err(error) => Err(error.into()),
    }
}
pub fn prepare() -> Result<()> {
    crate::desktop_access::ensure_compatible()?;
    install()?;
    if running()? {
        ensure_owned_runtime()?;
    }
    Ok(())
}
fn runtime_owned(active: bool, own: bool, foreign: bool) -> bool {
    !foreign && (!active || own)
}
fn ensure_owned_runtime() -> Result<()> {
    let expected = exe()?;
    let processes = crate::component_runtime::processes("breeze.exe")?;
    let own = processes
        .iter()
        .any(|path| crate::component_runtime::same_path(path, &expected));
    let foreign = processes
        .iter()
        .any(|path| !crate::component_runtime::same_path(path, &expected));
    if !runtime_owned(running()?, own, foreign) {
        bail!(
            "检测到其他来源的 Breeze 正在运行，请退出它后重试。WinGlow 自己启动的实例会自动复用。"
        );
    }
    Ok(())
}
pub fn pause_owned() -> Result<()> {
    if running()? {
        stop()?;
    }
    Ok(())
}
pub fn sync() -> Result<()> {
    if !enabled()? {
        return stop();
    }
    migrate_startup()?;
    resume_runtime()
}
fn migrate_startup() -> Result<()> {
    let old_direct = direct_command()?;
    let primary_is_old = WindowsRegistry
        .read(&slot())?
        .and_then(|value| registry::as_string(&value))
        .is_some_and(|value| needs_startup_migration(&value, &old_direct));
    let legacy_is_old = WindowsRegistry
        .read(&legacy_slot())?
        .and_then(|value| registry::as_string(&value))
        .is_some_and(|value| needs_startup_migration(&value, &old_direct));
    if primary_is_old || legacy_is_old {
        shell_engine::commit("menu:breeze", startup_entries()?, || Ok(()))?;
    }
    Ok(())
}
pub(crate) fn resume_runtime() -> Result<()> {
    prepare()?;
    if running()? {
        return Ok(());
    }
    let mut child = Command::new(exe()?)
        .arg("inject-consistent")
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    std::thread::sleep(Duration::from_millis(350));
    if let Some(status) = child.try_wait()? {
        bail!("Breeze 启动失败（{status}）。");
    }
    Ok(())
}
pub fn set(enable: bool) -> Result<ActionResult> {
    shell_engine::ensure_ready()?;
    if enable {
        prepare().context("无法准备 Breeze。")?;
        let exe = exe()?;
        let mut child = None;
        let mut entries = vec![Entry {
            slot: slot(),
            value: Some(registry::string(&command()?)),
        }];
        if WindowsRegistry
            .read(&legacy_slot())?
            .and_then(|v| registry::as_string(&v))
            .is_some_and(|s| s == command().unwrap_or_default())
        {
            entries.push(Entry {
                slot: legacy_slot(),
                value: None,
            });
        }
        entries.extend(crate::archive_filter::plan()?);
        let result = shell_engine::commit("menu:breeze", entries, || {
            if running()? {
                return ensure_owned_runtime();
            }
            let mut process = Command::new(&exe)
                .arg("inject-consistent")
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?;
            std::thread::sleep(Duration::from_millis(350));
            if let Some(status) = process.try_wait()? {
                bail!("Breeze 未保持运行（{status}）。请检查是否已由其他工具启动。");
            }
            child = Some(process);
            Ok(())
        });
        if result.is_err()
            && let Some(mut process) = child
        {
            let _ = process.kill();
        }
        result?;
        Ok(ActionResult {
            message: "Breeze 已启动。实际菜单效果请在资源管理器中查看。".into(),
        })
    } else {
        let mut entries = shell_engine::baseline(&[slot(), legacy_slot()])?;
        let expected = command()?;
        let old_direct = direct_command()?;
        for entry in &mut entries {
            if entry
                .value
                .as_ref()
                .and_then(registry::as_string)
                .is_some_and(|s| is_breeze_startup_command(&s, &expected, &old_direct))
            {
                entry.value = None;
            }
        }
        shell_engine::commit("menu:breeze", entries, stop)?;
        Ok(ActionResult {
            message: "已关闭 Breeze 自启和注入进程。注销后恢复原菜单。".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_launcher_runs_breeze_without_showing_a_console() {
        let script = hidden_launcher_script(std::path::Path::new(r"C:\Breeze\breeze.exe"));
        assert!(script.contains("inject-consistent"));
        assert!(script.contains(", 0, False"));
    }
    #[test]
    fn old_direct_startup_command_remains_recognized_for_migration() {
        assert!(is_breeze_startup_command(
            r#""C:\Breeze\breeze.exe" inject-consistent"#,
            "hidden launcher",
            r#""C:\Breeze\breeze.exe" inject-consistent"#
        ));
    }
    #[test]
    fn only_the_old_direct_command_triggers_startup_migration() {
        assert!(needs_startup_migration(
            r#""C:\Breeze\breeze.exe" inject-consistent"#,
            r#""C:\Breeze\breeze.exe" inject-consistent"#
        ));
        assert!(!needs_startup_migration(
            "hidden launcher",
            "old direct command"
        ));
    }
    #[test]
    fn runtime_ownership_does_not_depend_on_startup_toggle() {
        assert!(runtime_owned(true, true, false));
        assert!(runtime_owned(false, false, false));
        assert!(!runtime_owned(true, false, false));
        assert!(!runtime_owned(true, true, true));
        assert!(!runtime_owned(false, true, true));
    }
    #[test]
    #[ignore = "Read-only installed runtime acceptance"]
    fn real_owned_runtime_is_accepted_without_startup_registration() {
        assert!(running().unwrap());
        ensure_owned_runtime().unwrap();
    }
}
