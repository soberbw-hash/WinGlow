# WinGlow 3.1.2 本地验证

日期：2026-10-03。

## 本次变化

字体应用、普通字体还原、撤销最近修改成功后，自动重启当前桌面的 Windows 资源管理器，随后检查桌面是否重新出现。仅导入字体、操作失败以及 DISM/SFC 扫描修复不触发重启。安装程序本身不触发重启。

使用 GetShellWindow 定位当前桌面的进程，验证当前会话及 Windows explorer.exe 的完整路径。先准备挂起的替代进程，再停止原进程，启动替代进程；若 Windows 已自动恢复桌面，则销毁挂起的替代进程，避免额外打开文件窗口。普通启动保持用户权限；管理员启动采用原 Explorer 的令牌。单独的字体管理员 helper 不重启资源管理器。

刷新在字体事务提交之后执行。刷新失败会保留成功修改与备份，并报告任务管理器恢复方式。它不等于注册表修改失败，也不承诺所有应用都已更新字体。

桌面和任务栏会短暂消失，资源管理器窗口可能关闭，应用前应完成文件复制等操作。其他已打开应用需要重新打开，部分界面仍需要注销。

## 已通过的检查

- 25 项 Rust 测试通过，包括修改失败不触发刷新、刷新失败保留成功结果并报告恢复方式、导入和扫描修复不触发刷新。
- Rust 格式检查和严格 Clippy 检查通过。
- TypeScript 检查与 Vite 生产构建通过。
- NSIS 安装包与便携包生成成功，附 SHA-256 清单，位置为 `artifacts/WinGlow-3.1.2`；尚未发布 GitHub Release。
- 新 EXE 的 `--diagnose` 只读检查正常返回：当前方案为思源黑体，`canRestore=true`，无待恢复事务。未运行安装程序。

## 实机边界

自动测试未终止开发电脑的资源管理器，也未修改开发电脑的字体。完整重启、桌面和任务栏重现、实际字体效果、管理员启动时的重启行为仍需实机检查。

实机检查顺序：完成文件操作后应用一种字体，确认桌面与任务栏恢复；重新打开文件窗口检查字体；普通还原后重复检查；导入字体和取消 UAC 时确认不发生重启。检查备份仍可还原。系统扫描修复仍按其结果提示自行注销或重启电脑。

Win32 API 依据：[GetShellWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getshellwindow)、[CreateProcessWithTokenW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createprocesswithtokenw)、[TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess)。
