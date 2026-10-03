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
    time::Duration,
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
fn slot() -> registry::Slot {
    registry::slot(Scope::Startup, "WinGlow-Breeze")
}
fn legacy_slot() -> registry::Slot {
    registry::slot(Scope::Startup, "WindowsWeitiao-Breeze")
}
fn command() -> Result<String> {
    Ok(format!("\"{}\" inject-consistent", exe()?.display()))
}
pub fn enabled() -> Result<bool> {
    let expected = command()?;
    for candidate in [slot(), legacy_slot()] {
        if WindowsRegistry
            .read(&candidate)?
            .and_then(|v| registry::as_string(&v))
            .is_some_and(|s| s == expected)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn install() -> Result<PathBuf> {
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
    Ok(())
}
pub fn set(enable: bool) -> Result<ActionResult> {
    shell_engine::ensure_ready()?;
    if enable {
        if unsafe { windows::Win32::UI::Shell::IsUserAnAdmin() }.as_bool() {
            bail!("请以普通权限打开应用后启用 Breeze。");
        }
        let exe = install().context("无法准备 Breeze。")?;
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
        let result = shell_engine::commit("menu:breeze", entries, || {
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
        for entry in &mut entries {
            if entry
                .value
                .as_ref()
                .and_then(registry::as_string)
                .is_some_and(|s| s == expected)
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
