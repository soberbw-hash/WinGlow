use crate::{
    font_engine,
    models::{ActionResult, Operation, WorkerResponse},
    transaction,
};
use anyhow::{Context, Result, anyhow, bail};
use std::{env, ffi::OsStr, fs, os::windows::ffi::OsStrExt, path::Path};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
        Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
        System::Threading::{
            CreateMutexW, GetExitCodeProcess, INFINITE, ReleaseMutex, WaitForSingleObject,
        },
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_SHIFT},
            Shell::{
                IsUserAnAdmin, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
                ShellExecuteExW,
            },
            WindowsAndMessaging::{MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW, SW_HIDE},
        },
    },
    core::PCWSTR,
};

fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain([0]).collect()
}
pub(crate) struct OperationLock(HANDLE);
impl Drop for OperationLock {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}
pub(crate) fn lock() -> Result<OperationLock> {
    lock_named("Global\\WindowsWeitiaoFontTransaction")
}
pub(crate) fn optimization_lock() -> Result<OperationLock> {
    lock_named("Global\\WinGlowOptimization")
}
fn lock_named(mutex_name: &str) -> Result<OperationLock> {
    // Shared with previous builds so old and renamed clients cannot write concurrently.
    let name = wide(mutex_name);
    let handle = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) }?;
    let wait = unsafe { WaitForSingleObject(handle, 0) };
    if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
        unsafe {
            let _ = CloseHandle(handle);
        }
        bail!("另一项修改正在进行，请稍后再试。");
    }
    Ok(OperationLock(handle))
}

pub fn replace_file(source: &Path, target: &Path) -> Result<()> {
    let a = wide(source);
    let b = wide(target);
    unsafe {
        MoveFileExW(
            PCWSTR(a.as_ptr()),
            PCWSTR(b.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .context("保存字体记录失败。")
}

fn elevate(nonce: &str) -> Result<()> {
    // ShellExecuteEx is called on a blocking pool thread, which has no COM apartment.
    let com = unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        )
    };
    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    windows::Win32::System::Com::CoUninitialize();
                }
            }
        }
    }
    let _com = ComGuard(com.is_ok());
    let exe = wide(env::current_exe()?);
    let verb = wide("runas");
    let args = wide(format!("--font-worker {nonce}"));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(exe.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }
        .map_err(|e| anyhow!("未获得修改字体所需的管理员授权：{e}"))?;
    if info.hProcess.is_invalid() {
        bail!("无法等待字体修改进程。");
    }
    let wait = unsafe { WaitForSingleObject(info.hProcess, INFINITE) };
    let mut code = 1;
    let exit = unsafe { GetExitCodeProcess(info.hProcess, &mut code) };
    unsafe {
        let _ = CloseHandle(info.hProcess);
    }
    if wait != WAIT_OBJECT_0 {
        bail!("无法确认修改进程是否完成，请检查恢复页面。");
    }
    exit?;
    // Detailed operation errors are returned in response.json, independent of the process code.
    Ok(())
}

fn request_dir(nonce: &str) -> Result<std::path::PathBuf> {
    uuid::Uuid::parse_str(nonce).context("修改请求标识无效。")?;
    Ok(font_engine::data_root()?.join("Requests").join(nonce))
}

pub fn run(operation: Operation) -> Result<ActionResult> {
    let _optimization_guard = optimization_lock()?;
    let refresh = crate::explorer::needed(&operation);
    let sync_taskbar = matches!(&operation, Operation::RestoreCategory {category} if category == "details" || category == "all-last");
    let mut outcome = run_inner(operation);
    if sync_taskbar
        && let Ok(result) = &mut outcome
        && let Err(error) = crate::visual::sync()
    {
        result
            .message
            .push_str(&format!(" 视觉组件恢复未完成：{error:#}。"));
    }
    if sync_taskbar
        && let Ok(result) = &mut outcome
        && let Err(error) = crate::taskbar::sync()
    {
        result
            .message
            .push_str(&format!(" 透明任务栏刷新未完成：{error:#}。"));
    }
    crate::explorer::finish(outcome, refresh, || {
        let _guard = lock()?;
        crate::explorer::restart()
    })
}

pub(crate) fn run_inner(operation: Operation) -> Result<ActionResult> {
    let needs_admin = match &operation {
        Operation::OptionalTool { id, verb } => id == "explorer-patcher" && verb != "open",
        Operation::Toggle { .. } => false,
        Operation::Menu { id, .. } => crate::menu::needs_admin(id)?,
        Operation::RestoreCategory { category } => category == "menu" || category == "all-last",
        _ => true,
    };
    // Desktop and Breeze operations always run as the user; do not inject an elevated engine.
    if !needs_admin {
        let _guard = lock()?;
        return dispatch(operation);
    }
    if unsafe { IsUserAnAdmin().as_bool() } {
        let _guard = lock()?;
        return dispatch(operation);
    }
    let nonce = uuid::Uuid::new_v4().to_string();
    let dir = request_dir(&nonce)?;
    fs::create_dir_all(&dir)?;
    transaction::persist_new(&dir.join("request.json"), &serde_json::to_vec(&operation)?)?;
    let outcome = (|| {
        elevate(&nonce)?;
        let response: WorkerResponse = serde_json::from_slice(
            &fs::read(dir.join("response.json"))
                .context("修改进程未返回结果。请检查恢复页面后再操作。")?,
        )?;
        if let Some(error) = response.error {
            bail!("{error}");
        }
        response.result.ok_or_else(|| anyhow!("修改结果为空。"))
    })();
    // Exact own filenames only. Keep a request without response for diagnosis after a worker crash.
    if dir.join("response.json").exists() {
        let _ = fs::remove_file(dir.join("request.json"));
        let _ = fs::remove_file(dir.join("response.json"));
        let _ = fs::remove_dir(&dir);
    }
    outcome
}

fn worker(nonce: &str) -> Result<()> {
    if !unsafe { IsUserAnAdmin().as_bool() } {
        bail!("此修改进程需要管理员权限。");
    }
    let dir = request_dir(nonce)?;
    let request = dir.join("request.json");
    if fs::metadata(&request)?.len() > 65536 {
        bail!("修改请求过大。");
    }
    let operation: Operation = serde_json::from_slice(&fs::read(request)?)?;
    let outcome = (|| {
        let _guard = lock()?;
        dispatch(operation)
    })();
    let response = match outcome {
        Ok(result) => WorkerResponse {
            result: Some(result),
            error: None,
        },
        Err(e) => WorkerResponse {
            result: None,
            error: Some(format!("{e:#}")),
        },
    };
    transaction::persist_new(&dir.join("response.json"), &serde_json::to_vec(&response)?)?;
    Ok(())
}

fn dispatch(operation: Operation) -> Result<ActionResult> {
    match operation {
        Operation::OptionalTool { id, verb } => crate::optional_tools::perform(&id, &verb),
        Operation::Toggle { id, enabled } => crate::shell_engine::toggle(&id, enabled),
        Operation::Menu { id, enabled } => crate::shell_engine::toggle_menu(&id, enabled),
        Operation::RestoreCategory { category } => {
            if category == "all-last" {
                crate::shell_engine::undo()
            } else {
                crate::shell_engine::restore(&category)
            }
        }
        Operation::OptimizeCore { token } => crate::optimization::core(&token),
        Operation::UndoOptimization { token } => crate::optimization::undo_core(&token),
        Operation::OptimizeMenu => crate::optimization::menu_only(),
        Operation::Repair => crate::repair::run(),
        font_operation => font_engine::perform(font_operation),
    }
}

fn show_message(text: &str, error: bool) {
    let text = wide(text);
    let title = wide("WinGlow");
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK
                | if error {
                    MB_ICONERROR
                } else {
                    MB_ICONINFORMATION
                },
        );
    }
}

pub fn handle_cli() -> bool {
    let args: Vec<_> = env::args().collect();
    if args.iter().any(|a| a == "--verify-menu") {
        let result = (|| -> Result<()> {
            let dictionary_update = crate::menu_dictionary::refresh();
            let items = crate::menu::scan()?;
            fs::write(
                font_engine::data_root()?.join("menu-snapshot.json"),
                serde_json::to_vec(&items.iter().map(|i| &i.0).collect::<Vec<_>>())?,
            )?;
            let mut targets = std::collections::BTreeMap::<String, usize>::new();
            for item in &items {
                for target in &item.0.targets {
                    *targets.entry(target.clone()).or_default() += 1;
                }
            }
            let report = serde_json::json!({"success":true,"items":items.len(),"withIcons":items.iter().filter(|i| i.0.icon_data_url.is_some()).count(),"targets":targets,"dictionaryUpdated":dictionary_update.is_ok(),"dictionaryError":dictionary_update.err().map(|e|format!("{e:#}"))});
            fs::write(
                font_engine::data_root()?.join("menu-verification.json"),
                serde_json::to_vec_pretty(&report)?,
            )?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e:#}");
        }
        return true;
    }
    if args.iter().any(|a| a == "--verify-visual") {
        if let Err(e) = crate::visual_diagnostics::run() {
            eprintln!("{e:#}");
        }
        return true;
    }
    if args.iter().any(|a| a == "--window-material-host") {
        if let Err(e) = crate::window_material::host() {
            eprintln!("{e:#}");
        }
        return true;
    }
    if let Some(index) = args.iter().position(|a| a == "--font-worker") {
        if let Some(nonce) = args.get(index + 1)
            && let Err(e) = worker(nonce)
        {
            show_message(&format!("{e:#}"), true);
        }
        return true;
    }
    if args.iter().any(|a| a == "--diagnose") {
        match font_engine::load_bootstrap() {
            Ok(data) => println!(
                "{}",
                serde_json::to_string_pretty(&data).unwrap_or_default()
            ),
            Err(e) => eprintln!("{e:#}"),
        };
        return true;
    }
    if args.iter().any(|a| a == "--diagnose-shell") {
        match crate::shell_engine::load() {
            Ok(data) => println!(
                "{}",
                serde_json::to_string_pretty(&data).unwrap_or_default()
            ),
            Err(e) => eprintln!("{e:#}"),
        };
        return true;
    }
    if args.iter().any(|a| a == "--diagnose-ui-fonts") {
        match crate::ui_fonts::diagnostics() {
            Ok(data) => println!(
                "{}",
                serde_json::to_string_pretty(&data).unwrap_or_default()
            ),
            Err(e) => eprintln!("{e:#}"),
        };
        return true;
    }
    if args.iter().any(|a| a == "--diagnose-desktop") {
        match crate::desktop::flags() {
            Ok(flags) => println!("Desktop folder flags: {flags:#x}"),
            Err(e) => eprintln!("{e:#}"),
        };
        return true;
    }
    if args.iter().any(|a| a == "--repair-system") {
        match run(Operation::Repair) {
            Ok(result) => show_message(&result.message, false),
            Err(e) => show_message(&format!("{e:#}"), true),
        };
        return true;
    }
    if args.iter().any(|a| a == "--emergency-reset")
        || unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) } < 0
    {
        match run(Operation::Restore {
            mode: "last".into(),
        }) {
            Ok(result) => show_message(&result.message, false),
            Err(e) => show_message(&format!("急救恢复未完成：{e:#}"), true),
        }
        return true;
    }
    false
}
