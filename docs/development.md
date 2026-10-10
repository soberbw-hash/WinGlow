# WinGlow开发约定

主线为 WinGlow.App，Rust crate 为 winglow，程序为 WinGlow.exe。保留旧应用 identifier 以支持已安装版本升级；它是兼容标识，不是显示名称。根目录 C#/WinForms 工程为历史实现，不用于 3.x 打包。

## 产品方向

功能取舍以 [WinGlow 发展方向](product-direction.md) 为依据：优先完善字体优化，逐步扩展视觉美化与个性化设置，始终保持轻量、极简和可靠还原。发展目标与已实现、已验证的能力应分别记录。

## 技术底座

Rust stable，edition 2024，最低 1.88；Tauri 2.12.1 稳定版、React 19.3、TypeScript 7、Vite 8。版本由 Cargo.lock 与 package-lock.json 固定。系统绘制仍由 Windows 各应用的渲染器决定；应用框架升级不能代替系统字体适配。

- `src/App.tsx`：字体选择、前后预览、功能导航及恢复页。
- `src-tauri/src/font_engine.rs`：字体解析、资源准备、应用计划和状态读取。
- `ui_fonts.rs`：六种经典界面的实时字体与持久化 WindowMetrics 字体，系统 DPI 上下文与字号保留。
- `registry.rs`：限定可写注册表位置，保留原始数据类型与字节。
- `transaction.rs`：不可变前后快照、写入验证、提交标记和回退。
- `worker.rs`：管理员 helper、跨进程互斥、结果回传与 Shift 急救。
- `menu.rs` / `shell_engine.rs`：菜单枚举、开关与分类还原。
- `menu_icon.rs`：仅从本地 EXE/DLL/ICO 读取图标资源，32×32 PNG，按路径、资源索引、修改时间与大小缓存；不加载或执行菜单扩展。
- `src/components/MenuManager.tsx`：按右键位置筛选、紧凑列表与开关示意，浏览器模式禁用修改。
- `desktop.rs`：通过 COM 修改桌面文字显示，记录实时状态，不重命名文件。
- `explorer.rs`：字体应用、普通字体还原、撤销最近修改成功后重启当前桌面的资源管理器，并验证桌面重新出现。
- `breeze.rs`：固定版本外部引擎下载校验、匹配当前桌面权限的启动与关闭。
- `desktop_access.rs`：读取当前桌面与软件的账户 SID、完整性级别；同账户单独提权启动时自动使用桌面令牌重新打开界面，权限一致的管理员桌面允许运行。不修改 UAC；管理员 worker 与材质 host 不经过 GUI 重新启动分支。
- `repair.rs`：DISM / SFC 日志、失败报告与默认系统字体注册修复。
- `legacy.rs`：选择性读取 v2 REG 备份，不执行 reg import。
- `preset_data.rs`：唯一的生产字体选择与字重映射表。浏览器预览的三项模拟数据必须与此一致。

3.1.3 的三套候选字体均通过 include_bytes 内置，预览使用同一组字体文件的 @font-face。候选状态和映射从真实内部完整名称、字重读取，苹方的粗体映射到 Semibold 600；保留旧版导入数据与事务快照的恢复兼容性，主界面不再出现导入入口。来源、固定提交、哈希和版权声明随软件包保存。

原图、来源 JSON 与派生图标见 docs/branding，脚本 scripts/generate-brand-assets.py 统一生成。原图和来源随两种包保存。

WinGlow 改名保留四类兼容记录：旧用户数据目录、原应用 identifier、与旧版共享的互斥名称、旧 WindowsWeitiao-Breeze 注册值白名单。新启动项使用 WinGlow-Breeze，检测和关闭支持旧项；历史发行记录和已发布资源文件名保留原名，不能将旧链接改成不存在的资源。

浏览器模式只能查看界面，应用按钮禁用，不模拟 Windows 写入成功。

3.1.5 保留三种字体族，鸿蒙包含兼容原 ID 的标准方案 harmonyos-sc 和新增 harmonyos-sc-bold。粗方案让正文使用真实 Bold 700，预览通过原 Bold 文件单独的 CSS 族提供，避免伪加粗。标准保留 Regular/Medium/Bold 层级映射。当前预览的字体与字重读取 Windows 实时 MessageFont，不再仅由 FontSubstitutes 推断显示效果；needsFontRefresh 检查六种实时角色的族与字重，旧映射已应用但原生设置缺失时，允许同方案再次应用。

应用计划增加六个固定 WindowMetrics 字体值和一个 LiveUiFonts 虚拟值。持久值保留原类型、字节与不存在状态；虚拟值通过 SPI_GET 读取六种 LOGFONT 与系统 DPI，在同一事务中通过 SPI_SET 更新。SPI_SET 不使用 SPIF_UPDATEINIFILE，持久化由已有逐值事务控制，避免回退时把原本不存在的值重新创建。仅调整字体族与字重，字号、质量标记、窗口和菜单尺寸不作为美化参数修改；持久 LOGFONT 按 96 DPI 编码，实时快照保留原 DPI 下的精确值，写入时使用 System-aware 线程上下文。任何写入、激活或提交失败，同一事务回退实时值与持久值；普通默认恢复、最近修改撤销及字体修复会包含这些字体范围。不同 DPI 下恢复会换算字体高度，若回读不一致则明确报告，不伪装成成功。诊断 --diagnose-ui-fonts 仅查询，不设置。

3.1.4 菜单扫描新增固定文件类型白名单；修改路径沿用同一白名单，不开放任意注册表路径。菜单 ID 与备份位置兼容旧版，按 CLSID 合并扩展的作用位置，保留一个全局开关。读取嵌套 shell 和 CommandStore 的静态二级菜单；动态菜单不臆造内部项目。菜单名称解析系统资源及常见友好名称，原名收在详情中；图标来源区分菜单图标与程序图标。已知位置按文件类型继承公共菜单，不等于全部关联类型的完整枚举，也不等于某个文件的实际弹出菜单。

## 应用与恢复

1.1.2 的扩展开关同时管理扫描到的所有 HKCU/HKLM ContextMenuHandlers 引用，停用时在默认 CLSID 前加 `-`，启用时恢复 CLSID；保留 COM 类和其它软件配置。旧 Blocked-only 的 OFF 状态在下一次一键精简时补全停用引用，禁止重新启用用户隐藏的项目。引用的原始类型、字节和缺失状态包含在原有事务中，菜单还原与撤销均覆盖新增范围。成功切换、精简和菜单还原后，通过现有任务栏保护流程刷新当前 Explorer，避免已加载的处理器继续留在旧进程中。

压缩扩展按 360/CZIP、7-Zip、WinRAR 等来源识别并保留。Breeze 过滤器保留已有的一个通用压缩及一个解压命令，删除快捷格式、邮件和重复的“其他压缩命令”。普通文件/文件夹仅保留其实际具备的压缩命令；压缩包才具备解压命令，不生成无效的解压入口。关闭 Breeze 时原生压缩扩展仍可用，但不会执行 Breeze 的细粒度过滤。

`--verify-native-menu` 在独立进程中通过 IShellItem / IContextMenu 查询自有文件夹、文本和空 ZIP 的原生菜单，输出 `native-menu-verification.json`，不调用菜单命令。它不经过 Breeze 的渲染过滤，不能当作 Breeze 的视觉验收。`--compact-menu` 执行和界面一键精简相同的备份、授权、修改与刷新流程，结果保存在 `menu-compact-result.json`。

普通界面采用 asInvoker。字体应用/导入/恢复与 HKLM 菜单修改时启动同一 EXE 的管理员 helper。当前用户设置和桌面 COM 操作直接运行。Breeze 启用拒绝管理员进程。helper 只接受 UUID 请求编号，按固定用户数据目录读取结构化操作；字体文件仍需完整解析。多窗口修改由 Global 命名 mutex 串行化；保持旧 mutex 名称，避免旧版与 WinGlow 同时修改系统。

资源先校验并准备到 `%PROGRAMDATA%\WinGlow\Fonts`，不会覆盖 `C:\Windows\Fonts`。内容哈希避免覆盖其它字体文件。资源准备失败不写注册表。

资源管理器刷新在成功的注册表事务完成、管理员 helper 返回之后，由界面所在进程执行；修改失败、仅导入字体、系统扫描修复均不触发。只定位当前桌面 GetShellWindow 对应的进程，校验会话及 Windows explorer.exe 路径，不按名称结束全部 Explorer。先创建挂起的替代进程，再停止原进程、恢复替代进程，等待桌面重新出现；管理员启动时使用原 Explorer 的用户令牌创建替代进程。刷新失败保留已提交的修改与备份，单独报告任务管理器恢复办法，不伪装成字体事务失败或自动回滚。

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

自动测试使用内存注册表后端和隔离的原生 HKCU 临时测试键，注入写入失败、激活失败、备份失败及无声写入丢失；不会更改开发电脑的字体。另检查九个内置字体的字重/常用字形和旧版 REG 解析。系统字体实测单独记录，不能用浏览器截图代替。苹方内置检查见 verification-3.1.3.md，事务检查的详细边界见 verification-3.1.0.md。

## 3.2.0 一键优化

optimization.rs 以一个 OptimizeCore 注册/实时字体事务管理字体、菜单和两个引擎自启。父进程先准备资源，保存 one-click-session.json；核心写入后以普通权限启动引擎，最后写完成标记。失败尝试撤销指定 UUID 对应的核心快照，不用“最新记录”猜测目标。恢复未完成保留会话；重新打开可继续恢复。进程启动成功只代表保持运行，不证明真实渲染效果。

taskbar.rs 使用固定 SHA-256 的官方 TranslucentTB 2026.2 zip，逐一释放白名单八文件，原文件存在时逐字节校验。TaskbarConfig 是只访问本工具独立 settings.json 的虚拟备份范围，准确保存不存在状态与原字节。版本信息读取识别 Windows 11；进程枚举核对完整 EXE 路径，拒绝接管其他实例；正常关闭仅向本工具实例窗口发送 WM_CLOSE，并等待退出，不按进程名强杀。启动与恢复运行态在普通父进程执行。分类还原与最近撤销需要同步该用户运行态。

menu_policy.rs 读取 command/CLSID InprocServer32 来源、文件版本 CompanyName 或已识别产品，保留系统路径、Microsoft、核心/安全入口与未知用途。版本厂商不是签名可信度验证；策略是可解释的确定性规则，不宣传 AI，也不为了达到固定 90% 而关闭未知项。批量写入是一笔可还原事务。autoHide 仅为当前扫描建议，详情折叠展示。

本轮功能必须验证：核心失败、引擎激活失败、完成标记失败、回退失败，文件原字节与不存在状态恢复，以及“已优化/撤销/继续恢复”UI。Windows 原生 SPI SET、引擎启动/退出和真实应用后还原另做实机验证，浏览器预览不执行主机修改。

## 3.2.1 菜单与默认方案

默认选择为 harmonyos-sc-bold，既有显式应用方案仍回显。OptimizeCore 固定使用粗方案；IconOverrides 29、77 与字体、菜单、引擎配置同事务记录，撤销恢复原值/原类型/不存在状态。

菜单扫描递归读取父项下 shell 子项及 SubCommands 指向的 CommandStore 子项；不加载第三方扩展。MenuVerb 白名单只允许既有根下最多五层 shell/verb，拒绝 command/CLSID 路径及基本操作。CommandStoreVerb 只允许单个受约束名称下 LegacyDisable；共享子项开关作用于所有引用它的菜单。恢复类别包含这两类，旧备份保持兼容。动态扩展不伪造子项；ExtendedSubCommandsKey 引用的外部类别暂未展开。

精简使用一次无图标扫描，不对每项重新扫描；对启用的附加父项整组隐藏，保留父项时递归精简子项，按 Slot 去重。用户明确要求的项目优先于旧安全入口保留规则；仍保留基本操作及不能确认归属的项目。用户看到的数量不代表单次右键实际菜单数量。

实现依据：[Microsoft 静态级联菜单说明](https://learn.microsoft.com/en-us/windows/win32/shell/creating-static-cascading-menus)。动态 IExplorerCommand/COM 子项与静态注册项是不同机制，静态扫描不能声称枚举完整实时菜单。

## 3.2.2 运行时和验收修订

file-extensions 写 HideFileExt 后通过 ordinary parent 的 Explorer 刷新生效；details 还原也刷新。restart 包装器在桌面恢复后调用已启用效果的同步，不启用已关闭效果。任务栏 reload_owned 保留 settings.json 的精确字节，在旧运行时退出可能回写缓存配置后重新写回预期配置，再绑定新 Explorer；仅按完整路径关闭自有实例。Breeze 使用原 inject-consistent 生命周期，退出后同步自有启用实例。

应用存在期间以两秒间隔监测 shell PID，持有优化互斥锁后处理新 shell，跳过运行中操作及未完成恢复，失败最多三次；应用自己的刷新更新已观察 PID，避免重复恢复。不是独立驻留服务，关闭 WinGlow 后不保证监测外部重启。

open_backup 是无路径参数的 native command，后端计算固定备份目录，通过 OpenerExt 打开，不开放前端任意路径权限。旧错误来自 JS opener 的 ACL 拒绝。

transaction 保存快照并同步落盘后才写设置。repair 扫描前新增只读 checkpoint，捕获管理的字体映射、实时角色和已记录设置；无写入测试覆盖。它不是磁盘镜像，DISM/SFC 文件修复不在可逆设置备份范围。

自动精简使用纯 plan_from，测试覆盖先前手动关闭、关闭父项、保留项与重复执行后空计划；所有输出都是隐藏标记，不输出恢复删除值。真实图标依次来自注册图标、扩展 DLL、命令 EXE；拒绝网络/相对路径及通用脚本宿主资源，前端采用功能图标兜底。图标表示类型的兜底不冒充应用官方图标。

任务栏默认配置依据 [TranslucentTB 官方配置](https://github.com/TranslucentTB/TranslucentTB.github.io/blob/master/config.md)，desktop_appearance 使用 accent=blur、show_line=true。

## 3.2.3 Explorer / TranslucentTB 生命周期修复

上游 2026.2 在30秒内观察到两次 Explorer PID 变化时，弹出阻塞警告后 ExitProcess(1)：[ResetState 源码](https://github.com/TranslucentTB/TranslucentTB/blob/2026.2/TranslucentTB/taskbar/taskbarattributeworker.cpp#L1351-L1359)。3.2.2 的刷新后重连太晚，且已阻塞实例无法处理普通 WM_CLOSE。

新的 protected_refresh 顺序：pause_owned 保存独立配置精确字节，停止自有实例并恢复可能回写的原配置；restart_shell 刷新；wait_for_taskbar 确認 GetShellWindow 与 Shell_TrayWnd 同 PID 且 PID/HWND 连续稳定2秒；最后仅同步已启用的效果。暂停失败不终止 Explorer，刷新失败仍尝试恢复效果。只有完整恢复成功才更新观察 PID，失败保留 watcher 有限重试机会。

正常退出仍发送 WM_CLOSE。4秒无响应时，重新 OpenProcess 获得指定 PID 的查询/终止/等待句柄，QueryFullProcessImageNameW 核验 WinGlow 独立组件的完整路径，才通过该句柄 TerminateProcess 并等待退出；不按名称强杀，不操作其他来源，不修改上游保护或二进制。Windows 的自动 Shell 恢复先获得2秒机会，避免过早启动备用进程引入竞争。外部 PID 变化的 watcher 同样先暂停自有实例，再等待任务栏稳定，最多3次恢复；依赖 WinGlow 正在运行，不能保证由其他程序引起的极短间隔重启永不出现上游提示。

显式实机验收命令（会重启桌面两次，不属于默认测试）：
`cargo test --manifest-path WinGlow.App/src-tauri/Cargo.toml explorer::tests::real_desktop_two_refreshes_keep_owned_effects_and_config -- --ignored --exact --nocapture`
它要求任务栏已经启用，持有优化互斥锁，检查两次 shell PID 确实改变、30秒内完成、独立配置逐字节不变、已启用 Breeze/任务栏仍在运行且无自有可见警告对话框。不改字体、菜单或系统设置。
