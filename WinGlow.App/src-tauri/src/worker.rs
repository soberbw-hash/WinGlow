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
                IsUserAnAdmin, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
                SHELLEXECUTEINFOW, ShellExecuteExW,
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
    // Readers (including Breeze hot reload and virus scanners) can briefly
    // open the destination without FILE_SHARE_DELETE. Preserve atomic replace
    // and the transaction; do not delete the old file or weaken its ACL.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(750);
    loop {
        let result = unsafe {
            MoveFileExW(
                PCWSTR(a.as_ptr()),
                PCWSTR(b.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        match result {
            Ok(()) => return Ok(()),
            Err(error)
                if replace_retryable(error.code()) && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("保存设置文件失败：{}。", target.display()));
            }
        }
    }
}
fn replace_retryable(code: windows::core::HRESULT) -> bool {
    [5, 32, 33]
        .into_iter()
        .any(|error| code == windows::core::HRESULT::from_win32(error))
}

fn launch_directory(executable: &Path) -> Result<&Path> {
    if !executable.is_file() {
        bail!("未找到 WinGlow 程序，请重新打开软件或重新安装。");
    }
    let directory = executable
        .parent()
        .filter(|p| p.is_dir())
        .ok_or_else(|| anyhow!("WinGlow 程序目录不可用，请重新打开软件或重新安装。"))?;
    Ok(directory)
}

fn launch_error(error: windows::core::Error) -> anyhow::Error {
    if error.code().0 == 0x800704C7u32 as i32 {
        anyhow!("操作已取消，未进行修改。需要管理员权限才能完成此操作。")
    } else {
        anyhow!("无法启动管理员修改进程，请重新打开 WinGlow 后重试：{error}")
    }
}

fn elevate(nonce: &str) -> Result<()> {
    launch_worker(nonce, false)
}

fn launch_worker(nonce: &str, probe: bool) -> Result<()> {
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
    // Packaged launchers can virtualize AppData. The elevation broker sees the
    // physical filesystem, so resolve the loaded image through its file handle
    // and use a Shell-compatible path without an unnecessary verbatim prefix.
    let executable = dunce::canonicalize(env::current_exe()?)
        .context("无法确定 WinGlow 程序的真实位置，请重新打开软件或重新安装。")?;
    // Shell's runas can fail even when lpFile exists if the inherited CWD is
    // invalid (for example a launcher supplied a damaged Unicode path).
    // Resolve both inputs before invoking the shell; never inherit its CWD.
    let directory = wide(launch_directory(&executable)?);
    let exe = wide(&executable);
    let verb = wide("runas");
    let command = if probe {
        "--verify-worker-child"
    } else {
        "--font-worker"
    };
    let args = wide(format!("{command} {nonce}"));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(exe.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        lpDirectory: PCWSTR(directory.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(launch_error)?;
    if info.hProcess.is_invalid() {
        bail!("无法等待管理员修改进程。");
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
    run_with_refresh(operation, true)
}
pub(crate) fn run_deferred(operation: Operation) -> Result<ActionResult> {
    run_with_refresh(operation, false)
}
fn run_with_refresh(operation: Operation, allow_refresh: bool) -> Result<ActionResult> {
    let _optimization_guard = optimization_lock()?;
    let refresh = allow_refresh && crate::explorer::needed(&operation);
    let sync_taskbar = matches!(&operation, Operation::RestoreCategory {category} if category == "details" || category == "all-last" || category == "pending");
    let mut outcome = run_inner(operation);
    if sync_taskbar
        && let Ok(result) = &mut outcome
        && let Err(error) = crate::visual::reload()
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
        Operation::OptionalTool { .. } => false,
        Operation::Toggle { .. }
        | Operation::Appearance { .. }
        | Operation::TaskbarSettings { .. } => false,
        Operation::Menu { id, .. } => crate::menu::needs_admin(id)?,
        Operation::RestoreCategory { category } => {
            matches!(category.as_str(), "menu" | "all-last" | "pending")
        }
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
        Operation::Appearance { id, settings } => crate::visual::apply_settings(&id, settings),
        Operation::Menu { id, enabled } => crate::shell_engine::toggle_menu(&id, enabled),
        Operation::RestoreCategory { category } => {
            if category == "pending" {
                crate::shell_engine::recover_pending()
            } else if category == "all-last" {
                crate::shell_engine::undo()
            } else {
                crate::shell_engine::restore(&category)
            }
        }
        Operation::OptimizeCore { token } => crate::optimization::core(&token),
        Operation::UndoOptimization { token } => crate::optimization::undo_core(&token),
        Operation::RollbackUpgrade { token } => crate::optimization::rollback_upgrade_core(&token),
        Operation::OptimizeMenu => crate::optimization::menu_only(),
        Operation::TaskbarSettings { settings } => crate::desktop_settings::apply_taskbar(settings),
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
    if args.iter().any(|a| a == "--wallpaper-host") {
        if let Err(error) = crate::optional_tools::carousel_host() {
            eprintln!("壁纸轮播未完成：{error:#}");
        }
        return true;
    }
    if let Some(index) = args.iter().position(|a| a == "--verify-worker-child") {
        let result = (|| -> Result<()> {
            if !unsafe { IsUserAnAdmin().as_bool() } {
                bail!("验证进程未获得管理员权限。");
            }
            let nonce = args
                .get(index + 1)
                .ok_or_else(|| anyhow!("缺少验证标识。"))?;
            let dir = request_dir(nonce)?;
            if fs::read(dir.join("probe.json"))? != b"WinGlow worker launch probe v1" {
                bail!("验证请求无效。");
            }
            transaction::persist_new(
                &dir.join("probe-result.json"),
                &serde_json::to_vec(&serde_json::json!({
                    "admin":true, "workingDirectory":env::current_dir()?, "executable":env::current_exe()?
                }))?,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("{error:#}");
        }
        return true;
    }
    if args.iter().any(|a| a == "--verify-worker-launch") {
        let result = (|| -> Result<serde_json::Value> {
            let nonce = uuid::Uuid::new_v4().to_string();
            let dir = request_dir(&nonce)?;
            fs::create_dir_all(&dir)?;
            transaction::persist_new(&dir.join("probe.json"), b"WinGlow worker launch probe v1")?;
            launch_worker(&nonce, true)?;
            let result: serde_json::Value =
                serde_json::from_slice(&fs::read(dir.join("probe-result.json"))?)?;
            let _ = fs::remove_file(dir.join("probe.json"));
            let _ = fs::remove_file(dir.join("probe-result.json"));
            let _ = fs::remove_dir(dir);
            Ok(result)
        })();
        let report = match result {
            Ok(child) => {
                serde_json::json!({"success":true,"child":child,"systemSettingsChanged":false})
            }
            Err(error) => {
                serde_json::json!({"success":false,"error":format!("{error:#}"),"systemSettingsChanged":false})
            }
        };
        if let Ok(root) = font_engine::data_root() {
            let _ = fs::write(
                root.join("worker-launch-verification.json"),
                report.to_string(),
            );
        }
        return true;
    }
    if args.iter().any(|a| a == "--verify-native-menu") {
        if let Err(e) = crate::menu_diagnostics::run() {
            eprintln!("{e:#}");
        }
        return true;
    }
    if args.iter().any(|a| a == "--verify-wallpaper-carousel") {
        if let Err(error) = crate::optional_tools::diagnose_carousel() {
            eprintln!("{error:#}");
        }
        return true;
    }
    if args.iter().any(|a| a == "--verify-integrated-controls") {
        if let Err(error) = crate::optional_tools::diagnose_integrated_controls() {
            eprintln!("{error:#}");
        }
        return true;
    }
    if args.iter().any(|a| a == "--restore-wallpaper-checkpoint") {
        if let Err(error) = crate::optional_tools::recover_carousel_probe() {
            eprintln!("{error:#}");
        }
        return true;
    }
    if args.iter().any(|a| a == "--compact-menu") {
        let report = match run(Operation::OptimizeMenu) {
            Ok(result) => serde_json::json!({"success":true,"message":result.message}),
            Err(e) => serde_json::json!({"success":false,"error":format!("{e:#}")}),
        };
        if let Ok(root) = font_engine::data_root() {
            let _ = fs::write(root.join("menu-compact-result.json"), report.to_string());
        }
        return true;
    }
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
    if args.iter().any(|a| a == "--diagnose-wallpaper-rpc") {
        // Empty automation arguments exercise dispatch/acknowledgement without
        // changing wallpaper selection, sound, runtime state or the desktop.
        let report = match crate::lively_rpc::command(&[]) {
            Ok(()) => serde_json::json!({"success":true,"message":"壁纸控制接口已确认请求。"}),
            Err(error) => serde_json::json!({"success":false,"error":format!("{error:#}")}),
        };
        println!("{report}");
        return true;
    }
    if args.iter().any(|a| a == "--repair-ui-font-sizes") {
        let report = (|| -> Result<ActionResult> {
            let _guard = optimization_lock()?;
            crate::shell_engine::ensure_ready()?;
            font_engine::repair_ui_font_sizes()
        })();
        let value = match report {
            Ok(result) => serde_json::json!({"success":true,"message":result.message}),
            Err(error) => serde_json::json!({"success":false,"error":format!("{error:#}")}),
        };
        if let Ok(root) = font_engine::data_root() {
            let _ = fs::write(root.join("font-size-repair-result.json"), value.to_string());
        }
        println!("{value}");
        return true;
    }
    if args.iter().any(|a| a == "--apply-one-click") {
        let outcome =
            crate::desktop_access::ensure_compatible().and_then(|()| crate::optimization::apply());
        let report = match outcome {
            Ok(result) => serde_json::json!({"success":true,"message":result.message}),
            Err(error) => serde_json::json!({"success":false,"error":format!("{error:#}")}),
        };
        if let Ok(root) = font_engine::data_root() {
            let _ = fs::write(
                root.join("one-click-application-result.json"),
                report.to_string(),
            );
        }
        println!("{report}");
        return true;
    }
    if args.iter().any(|a| a == "--recover-pending-no-refresh") {
        let report = match crate::recover_pending_inner(false) {
            Ok(result) => serde_json::json!({"success":true,"message":result.message}),
            Err(error) => serde_json::json!({"success":false,"error":format!("{error:#}")}),
        };
        if let Ok(root) = font_engine::data_root() {
            let _ = fs::write(
                root.join("pending-recovery-result.json"),
                report.to_string(),
            );
        }
        println!("{report}");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_replace_waits_for_a_short_lived_reader_without_losing_old_data() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = env::temp_dir().join(format!("WinGlow-file-reader-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let target = root.join("config.js");
        let source = root.join("config.tmp");
        fs::write(&target, b"original").unwrap();
        fs::write(&source, b"updated").unwrap();
        let reader = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&target)
            .unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            drop(reader);
        });
        replace_file(&source, &target).unwrap();
        release.join().unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"updated");
        assert!(!source.exists());
        fs::remove_file(target).unwrap();
        fs::remove_dir(root).unwrap();
        assert!(replace_retryable(windows::core::HRESULT::from_win32(32)));
        assert!(!replace_retryable(windows::core::HRESULT::from_win32(3)));
    }

    #[test]
    fn shell_launch_uses_existing_executable_directory_with_unicode_and_spaces() {
        let directory = env::temp_dir().join(format!("WinGlow 启动测试 {}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("WinGlow.exe");
        fs::write(&executable, b"test fixture").unwrap();
        assert_eq!(launch_directory(&executable).unwrap(), directory);
        assert!(launch_directory(&directory).is_err());
        fs::remove_file(&executable).unwrap();
        assert!(launch_directory(&executable).is_err());
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn cancellation_and_launch_failure_have_distinct_operation_neutral_messages() {
        let cancelled = launch_error(windows::core::Error::from_hresult(windows::core::HRESULT(
            0x800704C7u32 as i32,
        )))
        .to_string();
        let missing_path = launch_error(windows::core::Error::from_hresult(
            windows::core::HRESULT(0x80070003u32 as i32),
        ))
        .to_string();
        assert!(cancelled.contains("操作已取消"));
        assert!(!missing_path.contains("操作已取消"));
        assert!(!cancelled.contains("字体") && !missing_path.contains("字体"));
    }
}
