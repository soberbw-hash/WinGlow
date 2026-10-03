# WinGlow

让 Windows 更合心意。调整字体、右键菜单与桌面细节。

3.1.2 本地测试版。三个分类都已接入操作；字体页只保留选择、前后比较与应用。公开发布以 [GitHub Releases](https://github.com/soberbw-hash/WinGlow/releases) 为准。

官方图标使用用户提供的原图，来源与原文件见 [品牌资源](docs/branding/README.md)。原图与来源随安装版及便携版保存。

## 发展方向

以 Windows 字体优化为核心，持续完善字体替换、渲染效果、预览与恢复体验，逐步扩展视觉美化和个性化设置，成为轻量、简单、安全、随时可恢复的 Windows 视觉调节工具。完整说明见 [产品发展方向](docs/product-direction.md)。

## 功能

- 字体：HarmonyOS Sans、思源黑体、苹方。前两套内置常规、中等、粗体；苹方由用户导入有权使用的静态 PingFang SC，至少含常规与粗体。安装包不附带苹方或 SF Pro。
- 右键菜单：真实菜单项的开关、查找、刷新；Breeze 美化开关。管理常见经典菜单注册项，不覆盖全部 Windows 11 新式菜单或 ContextMenuManager 的全部功能。
- 基础美化：隐藏快捷方式箭头、隐藏盾牌角标、隐藏桌面图标文字、隐藏桌面图标、显示文件扩展名。隐藏文字不改文件名；盾牌覆盖是实验功能，不关闭 UAC。
- 还原：默认字体、分类还原、撤销最近修改；普通还原无效时，可运行 Windows 扫描并修复。

## 使用与还原

Windows 10 / 11 x64，使用系统 WebView2。正常打开无需管理员权限，需要修改系统位置时才请求提权。字体应用、普通字体还原和撤销最近修改成功后，会自动重启当前桌面的 Windows 资源管理器，桌面和任务栏短暂消失，资源管理器窗口可能关闭，请先完成文件复制等操作。其他已打开的应用需重新打开；部分字体界面、覆盖图标和 Breeze 的完整切换仍可能需要注销。程序不会自动注销或重启电脑。

Breeze 首次开启时从官方 release 下载固定版本 0.1.34，并验证 SHA-256。它作为单独、未修改的上游进程运行；安装包不内置其 EXE/DLL。关闭后移除本工具添加的登录启动设置、停止注入进程，注销后原菜单才完整恢复。原有第三方 Breeze 配置仍保留。

每次写入前保存原值的类型、字节与“不存在”状态，写入后回读校验；失败自动回退，未完成事务阻止继续修改。桌面文字的实时状态也保存。为保证改名后仍可还原，备份与已导入资源继续使用旧版兼容目录；备份位于 `%LOCALAPPDATA%\WindowsFontTuner\Backups`，兼容旧版选择性 REG 恢复。恢复本身也会备份，支持撤销。

按住 Shift 启动或运行 `WinGlow.exe --emergency-reset` 可恢复最近的字体修改。无法正常进入界面时，可运行 `WinGlow.exe --repair-system`；它会请求管理员权限并执行同样的扫描修复流程。

扫描修复依次尝试 DISM RestoreHealth、SFC scannow，再恢复本工具管理的默认字体映射与校验过的 Windows 字体注册信息。保留完整日志、退出码和修复前快照。可显式绕过损坏的字体备份并隔离该备份，保留原文件；这不等于完成原快照的精确撤销。有效的未完成菜单/桌面事务不会被字体修复假装完成。日志位于 `%LOCALAPPDATA%\WindowsFontTuner\RepairLogs`。网络或系统源错误会明确报告。

字体使用安装与映射方式，不直接覆盖系统字体文件；部分 DirectWrite、WinUI、UWP 界面可能不响应映射。SFC 是系统文件修复，并不能保证所有第三方修改都可恢复。实际覆盖范围、Breeze 注入、盾牌显示、真实应用/还原仍需 Windows 实机验证；构建和自动测试通过不代表上述效果已全部验证。

## 开发

主线目录 `WinGlow.App/`：Rust + Tauri 2、React + TypeScript。根目录 C#/WinForms 工程保留为历史参考。

```powershell
cd WinGlow.App
npm ci
npm run tauri dev
```

生成安装包、便携包与 SHA-256 清单：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/Build-ModernPackages.ps1
```

- [开发与恢复约定](docs/development.md)
- [视频研究与字体边界](docs/font-research.md)
- [右键菜单研究](docs/context-menu-research.md)
- [WinGlow 改名验证记录](docs/verification-3.1.1.md)
- [应用后自动刷新验证记录](docs/verification-3.1.2.md)
- [字体授权与来源](字体授权说明.md)
- [外部引擎声明](licenses/THIRD-PARTY.txt)

软件主体 MIT；字体沿用各自许可证，外部 Breeze 使用 AGPL-3.0。
