//! Identify components by the opened process's full image path, never by name alone.
use anyhow::{Result, bail};
use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};
use windows::{
    Win32::{
        Foundation::CloseHandle,
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW,
                PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPMODULE,
                TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
    },
    core::{PCWSTR, PWSTR},
};

pub fn processes(name: &str) -> Result<Vec<PathBuf>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?;
    struct Guard(windows::Win32::Foundation::HANDLE);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _guard = Guard(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = Vec::new();
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(name) {
                let process = unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION,
                        false,
                        entry.th32ProcessID,
                    )
                }
                .map_err(|_| anyhow::anyhow!("无法确认已有 {name} 的来源，请先关闭该组件。"))?;
                let _process = Guard(process);
                let mut buf = vec![0u16; 32768];
                let mut size = buf.len() as u32;
                unsafe {
                    QueryFullProcessImageNameW(
                        process,
                        PROCESS_NAME_WIN32,
                        PWSTR(buf.as_mut_ptr()),
                        &mut size,
                    )
                }?;
                found.push(PathBuf::from(String::from_utf16_lossy(
                    &buf[..size as usize],
                )));
            }
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    Ok(found)
}
pub fn own_running(exe: &Path) -> Result<bool> {
    let name = exe
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("组件路径无效。"))?
        .to_string_lossy();
    Ok(processes(&name)?.iter().any(|p| same_path(p, exe)))
}
pub fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}
pub fn preflight(exe: &Path) -> Result<()> {
    let name = exe.file_name().unwrap().to_string_lossy();
    if processes(&name)?.iter().any(|p| !same_path(p, exe)) {
        bail!("已有其他来源的 {name} 正在运行，请先关闭它后再开启本工具的效果。");
    }
    Ok(())
}
pub fn command(exe: &Path) -> Command {
    let mut cmd = Command::new(exe);
    cmd.creation_flags(0x08000000);
    cmd
}

// Avoid depending on a process name for launching system utilities.
pub fn system_exe(name: &str) -> Result<PathBuf> {
    let mut buf = [0u16; 32768];
    let len =
        unsafe { windows::Win32::System::SystemInformation::GetSystemDirectoryW(Some(&mut buf)) }
            as usize;
    if len == 0 || len >= buf.len() {
        bail!("无法定位 Windows 系统目录。");
    }
    let _ = PCWSTR::null();
    Ok(PathBuf::from(String::from_utf16_lossy(&buf[..len])).join(name))
}

pub fn loaded_module(process_name: &str, module_name: &str) -> Result<bool> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?;
    let mut p = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let result = (|| {
        if unsafe { Process32FirstW(snapshot, &mut p) }.is_ok() {
            loop {
                let len = p
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(p.szExeFile.len());
                if String::from_utf16_lossy(&p.szExeFile[..len]).eq_ignore_ascii_case(process_name)
                    && let Ok(modules) = unsafe {
                        CreateToolhelp32Snapshot(
                            TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                            p.th32ProcessID,
                        )
                    }
                {
                    let mut m = MODULEENTRY32W {
                        dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
                        ..Default::default()
                    };
                    let mut loaded = false;
                    if unsafe { Module32FirstW(modules, &mut m) }.is_ok() {
                        loop {
                            let len = m
                                .szModule
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(m.szModule.len());
                            if String::from_utf16_lossy(&m.szModule[..len])
                                .eq_ignore_ascii_case(module_name)
                            {
                                loaded = true;
                                break;
                            }
                            if unsafe { Module32NextW(modules, &mut m) }.is_err() {
                                break;
                            }
                        }
                    }
                    unsafe {
                        let _ = CloseHandle(modules);
                    }
                    if loaded {
                        return Ok(true);
                    }
                }
                if unsafe { Process32NextW(snapshot, &mut p) }.is_err() {
                    break;
                }
            }
        }
        Ok(false)
    })();
    unsafe {
        let _ = CloseHandle(snapshot);
    }
    result
}
