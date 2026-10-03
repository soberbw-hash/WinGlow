# WinGlow开发约定

主线为 WinGlow.App，Rust crate 为 winglow，程序为 WinGlow.exe。保留旧应用 identifier 以支持已安装版本升级；它是兼容标识，不是显示名称。根目录 C#/WinForms 工程为历史实现，不用于 3.x 打包。

## 技术底座

Rust stable，edition 2024，最低 1.88；Tauri 2.12.1 稳定版、React 19.3、TypeScript 7、Vite 8。版本由 Cargo.lock 与 package-lock.json 固定。系统绘制仍由 Windows 各应用的渲染器决定；应用框架升级不能代替系统字体适配。

- `src/App.tsx`：字体选择、前后预览、功能导航及恢复页。
- `src-tauri/src/font_engine.rs`：字体解析、资源准备、应用计划和状态读取。
- `registry.rs`：限定可写注册表位置，保留原始数据类型与字节。
- `transaction.rs`：不可变前后快照、写入验证、提交标记和回退。
- `worker.rs`：管理员 helper、跨进程互斥、结果回传与 Shift 急救。
- `menu.rs` / `shell_engine.rs`：菜单枚举、开关与分类还原。
- `desktop.rs`：通过 COM 修改桌面文字显示，记录实时状态，不重命名文件。
- `breeze.rs`：固定版本外部引擎下载校验、普通权限启动与关闭。
- `repair.rs`：DISM / SFC 日志、失败报告与默认系统字体注册修复。
- `legacy.rs`：选择性读取 v2 REG 备份，不执行 reg import。
- `preset_data.rs`：唯一的生产字体选择与字重映射表。浏览器预览的三项模拟数据必须与此一致。

原图、来源 JSON 与派生图标见 docs/branding，脚本 scripts/generate-brand-assets.py 统一生成。原图和来源随两种包保存。

WinGlow 改名保留四类兼容记录：旧用户数据目录、原应用 identifier、与旧版共享的互斥名称、旧 WindowsWeitiao-Breeze 注册值白名单。新启动项使用 WinGlow-Breeze，检测和关闭支持旧项；历史发行记录和已发布资源文件名保留原名，不能将旧链接改成不存在的资源。

浏览器模式只能查看界面，应用按钮禁用，不模拟 Windows 写入成功。

## 应用与恢复

普通界面采用 asInvoker。字体应用/导入/恢复与 HKLM 菜单修改时启动同一 EXE 的管理员 helper。当前用户设置和桌面 COM 操作直接运行。Breeze 启用拒绝管理员进程。helper 只接受 UUID 请求编号，按固定用户数据目录读取结构化操作；字体文件仍需完整解析。多窗口修改由 Global 命名 mutex 串行化；保持旧 mutex 名称，避免旧版与 WinGlow 同时修改系统。

资源先校验并准备到 `%PROGRAMDATA%\WinGlow\Fonts`，不会覆盖 `C:\Windows\Fonts`。内容哈希避免覆盖其它字体文件。资源准备失败不写注册表。

每次修改将所有受影响值的原类型、字节与“不存在”状态持久保存到 `%LOCALAPPDATA%\WindowsFontTuner\Backups\font-v3-*\snapshot.json`，然后写入、回读校验并创建 `committed` 标记。出错时恢复原值并验证，成功回退创建 `rolled-back`。没有提交或回退标记的快照在界面显示恢复入口，阻止继续叠加修改。

恢复本身也备份，因此可以撤销一次恢复。恢复默认清除管理的字体映射，复原记录过的 FontLink，以及旧版备份中的桌面平滑/WPF 参数；保留注册的候选字体资源，不删除缓存中可能被其它进程使用的文件。v3 不修改 ClearType/WPF 参数；若首次恢复的是 v2 REG 备份，则选择性恢复当时的字体映射、FontLink、桌面字体平滑和 DISPLAY 的 WPF 四个参数。旧字体安装记录不迁移。

普通恢复不能复原没有备份的第三方系统字体文件覆盖。独立扫描修复运行 DISM、SFC，再恢复默认映射与校验后的系统注册项；不会自动重启。备份损坏会报告，只有用户显式扫描修复可隔离损坏/未完成的字体快照，保留源文件；不将有效菜单/桌面待恢复事务标为已还原。系统工具失败和实际显示未验证不能当作全部修复成功。

## 验证与打包

```powershell
cd WinGlow.App
npm ci
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --bundles nsis
```

`scripts/Build-ModernPackages.ps1` 运行这些检查并生成安装包、便携包和 SHA-256 清单。公开发布前先完成 [字体实测](font-research.md) 中的真实系统检查；本地构建成功不能代表已发布。

自动测试使用内存注册表后端和隔离的原生 HKCU 临时测试键，注入写入失败、激活失败、备份失败及无声写入丢失；不会更改开发电脑的字体。另检查六个内置字体的字重/常用字形和旧版 REG 解析。系统字体实测单独记录，不能用浏览器截图代替。详细边界见 verification-3.1.0.md。
