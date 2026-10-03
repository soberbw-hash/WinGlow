# WinGlow

让 Windows 更合心意。调整字体、右键菜单与桌面细节。

3.2.2 本地测试版。首页一键优化字体、精简和美化右键菜单，并启用 Windows 11 透明任务栏；可一键撤销。三套字体内置，鸿蒙仅标准与粗两档。公开发布以 [GitHub Releases](https://github.com/soberbw-hash/WinGlow/releases) 为准。

官方图标使用用户提供的原图，来源与原文件见 [品牌资源](docs/branding/README.md)。原图与来源随安装版及便携版保存。

## 发展方向

以 Windows 字体优化为核心，持续完善字体替换、渲染效果、预览与恢复体验，逐步扩展视觉美化和个性化设置，成为轻量、简单、安全、随时可恢复的 Windows 视觉调节工具。完整说明见 [产品发展方向](docs/product-direction.md)。

## 功能

- 一键优化：首页一个按钮。默认鸿蒙粗体，隐藏快捷方式箭头与盾牌角标，批量精简附加菜单，启动 Breeze 与独立 TranslucentTB。完成后出现“撤销优化”，恢复这一轮优化前的设置；不抹掉用户本来的个性化设置。

- 字体：HarmonyOS Sans、思源黑体、苹方。鸿蒙只增加“标准 / 粗”两个选项，粗使用真实 Bold 700，预览与应用一致。三套均内置常规、中等与粗体字重；苹方使用 Windows 适配版 Regular、Medium、Semibold。来源与字体声明见 [字体资源说明](字体授权说明.md)。不附带 SF Pro。
- 右键菜单：大按钮“一键精简”；桌面、文件夹、EXE、图片等位置直接点选。删除右侧说明栏，子菜单点击展开并缩进，静态子项可独立开关；程序动态扩展使用整组开关。扩大精简范围，明确包含 Defender 扫描入口、百度网盘、夸克网盘、WorkBuddy 和图片转 PDF；不关闭 Defender 防护。保留打开、删除、重命名、属性等基本操作，隐藏项可手动重新开启并支持还原。
- 基础美化：独立透明任务栏开关（内附 TranslucentTB 2026.2，Windows 11 x64）；隐藏快捷方式箭头、隐藏盾牌角标、隐藏桌面图标文字、隐藏桌面图标、显示文件扩展名。隐藏文字不改文件名；盾牌覆盖是实验功能，不关闭 UAC。
- 还原：默认字体、分类还原、撤销最近修改；普通还原无效时，可运行 Windows 扫描并修复。

## 使用与还原

Windows 10 / 11 x64，使用系统 WebView2。正常打开无需管理员权限，需要修改系统位置时才请求提权。字体应用、普通字体还原和撤销最近修改成功后，会自动重启当前桌面的 Windows 资源管理器，桌面和任务栏短暂消失，资源管理器窗口可能关闭，请先完成文件复制等操作。其他已打开的应用需重新打开；部分字体界面、覆盖图标和 Breeze 的完整切换仍可能需要注销。程序不会自动注销或重启电脑。

Breeze 首次开启时从官方 release 下载固定版本 0.1.34，并验证 SHA-256。它作为单独、未修改的上游进程运行；安装包不内置其 EXE/DLL。关闭后移除本工具添加的登录启动设置、停止注入进程，注销后原菜单才完整恢复。原有第三方 Breeze 配置仍保留。

每次写入前保存原值的类型、字节与“不存在”状态，写入后回读校验；失败自动回退，未完成事务阻止继续修改。桌面文字的实时状态也保存。为保证改名后仍可还原，备份与已导入资源继续使用旧版兼容目录；备份位于 `%LOCALAPPDATA%\WindowsFontTuner\Backups`，兼容旧版选择性 REG 恢复。恢复本身也会备份，支持撤销。

按住 Shift 启动或运行 `WinGlow.exe --emergency-reset` 可恢复最近的字体修改。无法正常进入界面时，可运行 `WinGlow.exe --repair-system`；它会请求管理员权限并执行同样的扫描修复流程。

扫描修复依次尝试 DISM RestoreHealth、SFC scannow，再恢复本工具管理的默认字体映射与校验过的 Windows 字体注册信息。保留完整日志、退出码和修复前快照。可显式绕过损坏的字体备份并隔离该备份，保留原文件；这不等于完成原快照的精确撤销。有效的未完成菜单/桌面事务不会被字体修复假装完成。日志位于 `%LOCALAPPDATA%\WindowsFontTuner\RepairLogs`。网络或系统源错误会明确报告。

字体使用安装、名称映射及 Windows 原生界面字体设置，不直接覆盖系统字体文件。原生设置针对经典菜单、对话框、标题、状态栏和桌面文字，保留字号、平滑参数和界面尺寸；持久注册值与实时字体都备份并回读校验。旧版本只应用过映射时，可以重新应用同一方案。“更换前”参考当前 Windows 对话框字体；各应用自行指定的字体仍可能不同。部分 DirectWrite、WinUI、UWP 界面不响应这些设置；Breeze 自绘菜单使用独立字体文件与绘制引擎，不会因本工具应用字体而自动改变其配置。SFC 是系统文件修复，并不能保证所有第三方修改都可恢复。实际覆盖范围、Breeze 注入、盾牌显示、真实应用/还原仍需 Windows 实机验证；构建和自动测试通过不代表上述效果已全部验证。

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
- [苹方内置验证记录](docs/verification-3.1.3.md)
- [右键菜单分类验证记录](docs/verification-3.1.4.md)
- [一键优化、双字重与恢复验证记录](docs/verification-3.2.2.md)
- [热门美化工具候选清单](docs/customization-candidates.md)
- [字体授权与来源](字体授权说明.md)
- [外部引擎声明](licenses/THIRD-PARTY.txt)

软件主体 MIT；字体沿用各自许可证，外部 Breeze 使用 AGPL-3.0，独立 TranslucentTB 使用 GPL-3.0。

3.2.2 验收修订：主页统一内置鸿蒙字体、图标标题按钮共用中轴，适配高 DPI；任务栏默认模糊并显示细线。菜单精简覆盖 BitLocker 入口、旧版 Media Player、华硕、ToDesk、英伟达、豆包、ChatGPT 项目入口；只隐藏、不重新开启已关闭项目。优先读取真实程序图标，无法提取时使用对应功能图标。扩展名开关完成后自动刷新资源管理器，删除桌面图标文字隐藏入口，修复打开备份权限错误。操作自动备份；备份失败则停止修改。
