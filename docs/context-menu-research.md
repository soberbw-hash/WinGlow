# 右键菜单与桌面细节研究

2026-10-03。研究上游文档和代码，区分本工具实现与外部引擎效果。

## 选择

| 项目 | 适合的用途 | 本版本决定 |
| --- | --- | --- |
| [ContextMenuManager](https://github.com/BluePointLilac/ContextMenuManager) | 丰富的菜单项管理，采用 GPL-3.0 | 参考启用/禁用的交互；自行实现注册表扫描与事务，不复制代码 |
| [Breeze](https://github.com/std-microblock/breeze-shell) | 右键菜单外观与交互，采用 AGPL-3.0 | 独立运行未经修改的官方 0.1.34 引擎，一个开关控制 |
| [Nilesoft Shell](https://github.com/moudey/Shell) | 脚本化定制、菜单重组 | 更适合需要编写配置的用户；本版本暂不引入第二个菜单引擎 |

“最佳”取决于用途。这一版优先简单操作和可撤销，而非追求最多设置。

## 本工具的管理部分

读取 HKCU/HKLM 中文件、文件与文件夹、文件夹、文件夹空白处、磁盘和桌面的 shell verbs 与 ContextMenuHandlers。读取时不加载第三方扩展 DLL。静态项用 LegacyDisable 控制；动态扩展用 Shell Extensions\Blocked 的 CLSID 控制，同一个扩展只显示一个开关。启用时也检查并处理 HKLM 禁用值，涉及该位置才提权。打开、删除、重命名、属性等核心 verb 不供关闭。

所有写入沿用逐值备份和校验。不删菜单注册树，不执行用户输入的注册表路径。不覆盖所有 Windows 11 新式菜单、按文件类型菜单、新建、发送到、WinX，也不宣称拥有 ContextMenuManager 的全部功能。

## Breeze 接入和还原

校验 release 0.1.34 的 windows-build.zip，仅提取 x64/releasedbg/breeze.exe 与 shell.dll。完整 SHA-256 与来源见 licenses/THIRD-PARTY.txt。许可证全文一并保留。安装包不附带这两个文件，首次由用户开启时从官方地址下载。使用普通用户权限执行 inject-consistent；以管理员打开本工具时，启用会明确拒绝。

本工具仅写自己的 WinGlow-Breeze 登录启动值，不接管原有 breeze-shell 启动项。关闭时通过上游公开命名事件停止注入进程并还原自启值。已经载入 Explorer 的 DLL 不能因停止进程而立即卸载，因此需要注销；本工具不会杀 Explorer 或自动注销。关闭后保留下载缓存与上游配置，避免删除第三方仍在使用的文件。

上游启动成功不等于注入所有 Explorer 窗口成功。提示明确要求到资源管理器查看效果，不能将进程存活作为菜单美化成功的实机证据。

## 桌面细节

- 图标文字：使用 [IFolderView2](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifolderview2-setcurrentfolderflags) 的 FWF_HIDEFILENAMES，不改文件名；仅改变对应显示位。快照同时记录实时文字显示状态与持久化值不存在的状态。
- 快捷方式箭头、盾牌：使用当前用户 Shell Icons 值引用本工具的透明 ICO，不改 EXE 或关闭 UAC。盾牌覆盖标记为实验功能，Windows 版本可能不支持。
- 另有隐藏桌面图标与显示扩展名开关。
- [SHChangeNotify](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shchangenotify) 用于通知资源管理器变化；图标覆盖缓存仍可能要求注销。操作成功指值已写入并校验，不能保证任意缓存窗口立即重绘。

## 强制修复

按照 [Microsoft 系统组件修复流程](https://learn.microsoft.com/en-us/troubleshoot/windows-server/installing-updates-features-roles/fix-windows-update-errors) 运行 DISM RestoreHealth，然后 SFC scannow，保存两者的输出与退出码，再恢复本工具管理的默认字体映射和已验证名称的 Windows 原有字体注册项。

DISM 失败也继续尝试 SFC；最终报告失败，不将非零错误码当作修复成功。不自动重启。日志完成或工具退出为 0，不证明所有第三方修改都已修复；实际显示需注销后检查。没有可用源、系统文件仍缺失、字体名称异常等情况必须报告，不能保证无条件恢复。
