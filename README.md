# Windows 微调

让 Windows 更合心意。调整字体、右键菜单与界面细节。

3.0.0 本地测试版重做了字体页面与底层操作流程：选字体、比较更换前后、应用。右键菜单和界面细节目前只有导航入口，功能后续实现。公开发布版本以 [GitHub Releases](https://github.com/soberbw-hash/WindowsFontTuner/releases) 为准。

## 字体

- HarmonyOS Sans：内置常规、中等、粗体。
- 思源黑体：内置常规、中等、粗体，供比较测试。
- 苹方：自行导入有权使用的静态 PingFang SC 字体，至少包含常规和粗体。导入后先预览，再应用；安装包不附带苹方或 SF Pro。

主界面不提供渲染参数、场景切换或推荐分数。前后预览比较字形，不能代替真实 Windows 界面的效果验证。

## 使用

支持 Windows 10 / 11 x64，使用系统 WebView2。打开程序无需管理员权限；导入、应用和恢复时会请求管理员权限。应用后重新打开目标应用，部分界面需要注销后生效；程序不会关闭你的应用或自动注销。

在「恢复与设置」中可恢复上次修改、恢复默认映射、打开备份。按住 Shift 启动，或运行 `windowsfonttuner2.exe --emergency-reset`，恢复最近一次修改前的设置。备份损坏或不存在时会报告错误。

每次修改前记录受影响注册表值的原类型、内容及不存在状态，写入后回读校验。失败自动回退；发现未完成事务时先恢复再继续。备份保存在 `%LOCALAPPDATA%\WindowsFontTuner\Backups`，沿用旧版位置并支持选择性读取旧版 REG 备份。

此版本安装和映射候选字体，保留 ClearType 参数、Emoji 与图标字体。它不会替换 Windows 系统字体文件。硬编码字体及部分 DirectWrite、WinUI、UWP 界面可能不响应映射；不能保证达到系统文件替换教程的覆盖范围。默认恢复不修复第三方覆盖或损坏的系统字体文件。

## 开发

主线目录：`WindowsFontTuner2/`。Rust + Tauri 2，React + TypeScript；根目录 C#/WinForms 文件及旧版打包脚本保留为历史参考。

```powershell
cd WindowsFontTuner2
npm ci
npm run tauri dev
```

检查并生成安装版、便携包与 SHA-256 清单：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/Build-ModernPackages.ps1
```

- [开发与恢复约定](docs/development.md)
- [视频研究与字体优化边界](docs/font-research.md)
- [字体授权与来源](字体授权说明.md)

软件代码采用 MIT；字体沿用各自的许可证。
