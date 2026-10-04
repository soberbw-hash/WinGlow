# WinGlow 1.1.0 视觉集成

更新：2026-10-04。窗口磨砂与开始菜单美化加入一键优化，动态壁纸及经典布局仅作为可选功能。

| 功能 | 实现与默认效果 | 一键优化 |
| --- | --- | --- |
| 窗口磨砂 | Windows 官方 Desktop Acrylic 接口；跟随主题，保留已有材质，主要影响标准标题栏 | 是，Windows 11 22H2 起 |
| 开始菜单美化 | 隔离 Windhawk 1.7.3 + Start Menu Styler 1.7；主题亚克力、16 圆角、局部鸿蒙 SemiBold；保留布局 | 是，Windows 11 x64；其他 Windhawk 已运行时跳过 |
| 动态壁纸 | Lively 2.2.1.0；静音、全屏及电池暂停；打开原生壁纸选择界面 | 否，主动安装 |
| 经典布局 | ExplorerPatcher 26100.8457.70.3；按已支持 build 检查；不更改智能应用控制 | 否，主动安装 |

DWMBlurGlass 2.3.1r 需要私有 DWM 注入及符号。本机 Windows 26H1 build 28000.3086 未完成该组件兼容验证，因此没有打包或安装 DWMBlurGlass。公开 DWM 接口提供较有限的材质效果，不能承诺改变应用自绘内容。后续接入必须单独验证符号、卸载和 Windows 更新后的恢复。

## 恢复边界

- 外观配置与自启入口先进入自动事务备份；激活失败回滚，恢复失败保留可重试记录。旧 1.0.0 优化记录不会误认新增组件。
- 窗口原材质记录在 HWND 属性中，关闭时恢复；属性随窗口销毁而销毁，不会误用到新窗口。应用随后自行修改材质时保留应用的选择。
- Windhawk 只加载开始菜单模块，不修改用户另外安装的引擎或模块。任务栏继续由 TranslucentTB 管理。
- 可选工具不进入一键优化，不覆盖原有安装。WinGlow 自有安装才提供卸载还原；保留壁纸库、媒体及共享运行库。
- 安装、打开及卸载前自动备份固定配置。ExplorerPatcher 保存配置树和原本是否存在；Lively 保存设置和屏幕壁纸布局文件。经典布局刷新 Explorer 前暂停自有 TranslucentTB，桌面稳定后恢复。
- 仅下载官方固定版本与 SHA-256。管理员执行的 ExplorerPatcher 安装器放入管理员目录。

## 验证状态

本机 28000 已验证实际 Start Menu Styler DLL 加载、DWM 属性 0→3→0、后台跨进程应用/暂停还原/再次启用，以及新增配置与启动入口精确还原。两个可选安装包已实际下载并匹配固定 SHA-256；命令依据固定版本上游源码。可选工具完整安装/卸载与多屏壁纸视觉效果尚需实机验收。

原生验证入口 `WinGlow.exe --verify-visual` 暂时启用两个自动组件后恢复原配置，结果写入本地数据目录 `visual-verification.json`。正常启动不会运行该测试。

## 官方依据

- [Windows DWM 接口](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type)
- [Windhawk](https://github.com/ramensoftware/windhawk/releases/tag/v1.7.3)、[组件来源](../third-party/Windhawk/SOURCE.md)
- [Lively 命令控制](https://github.com/lively-community/lively/wiki/Command-Line-Controls)
- [ExplorerPatcher 固定发行版](https://github.com/valinet/ExplorerPatcher/releases/tag/26100.8457.70.3)
- [DWMBlurGlass](https://github.com/Maplespe/DWMBlurGlass)
