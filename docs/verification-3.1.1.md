# WinGlow 3.1.1 品牌改名验证

2026-10-03。本版修改名称、图标、界面色彩与旧名称兼容，不改动用户系统设置。

- 本地仓库目录改为 WinGlow，主线目录 WinGlow.App，历史 C# 工程 WinGlow.Legacy.csproj；Rust crate winglow、前端包 winglow、最终程序 WinGlow.exe。
- GitHub 仓库已改为 soberbw-hash/WinGlow，简介和本地 origin 均更新；未发布新 Release，未把本地版本写成已发布版本。
- 用户图标原文 SHA-256：9655572d74366d4c0ff370650c45e2ff20d307d9a2c1c4e58e2d69a5f2dc4ac0。原图 RGBA 1254×1254；UI PNG 为原图直接缩放，透明度保留，无裁切、重画或改色。
- ICO 包含 16、20、24、32、40、48、64、128、256 九个尺寸；Tauri PNG/ICNS、历史工程 ICO、网页 favicon 统一从同一原图生成，旧绘图脚本已改为调用唯一脚本。
- 原图和来源 JSON 在源码 docs/branding，并随安装版/便携版放在 branding；不得以派生低分辨率图标覆盖原图。
- Rust fmt、严格 Clippy、22 项测试、TypeScript 与 Vite 构建通过。
- 历史 C# 工程重新编译通过；现有环境缺 .NET 4.8 引用程序集并有架构警告，历史工程不作为本次正式测试包。
- 浏览器检查三种字体、三类导航、还原页和五个基础开关；1080×740 与 760×600 字体页无溢出，无控制台错误，无旧品牌显示。
- 保留旧用户数据目录、安装 identifier、与旧版共享的事务互斥名、旧 Breeze 启动项白名单，以保证已有备份/导入资源能继续读取。新 Breeze 启动项为 WinGlow-Breeze；关闭兼容旧项。历史包名称、旧资源 URL 的文件名仍按历史事实保留。

本版没有执行字体应用、菜单/美化开关、Breeze 注入、DISM/SFC 或安装升级。既有实机验证边界见 verification-3.1.0.md。

## 最终包检查

- Tauri release / NSIS 与便携打包通过；程序属性 ProductName / FileDescription 均为 WinGlow，版本 3.1.1。
- 无需执行 EXE 的 PE 资源检查通过：WinGlow.exe 与 Setup.exe 都有九个尺寸图标，全部原始图标 payload 与生成 ICO 完全一致。
- 最终 release 只读诊断仍为思源黑体、可还原、无待恢复事务，读取 62 个菜单项。与改名前 JSON 完全相同，旧备份可读取。
- Setup.exe：73,689,711 字节，SHA-256 96222d8ce5828856f99f66b85d40446423d445fbebef18a5487d74996853195e。
- Portable.zip：81,058,634 字节，SHA-256 30426802749a3de5bbe2b0d32267acdb6abee562073cc44a6a06e2a2663e04f6。
- GitHub 原仓库地址通过 API 返回 soberbw-hash/WinGlow，既有 v2.2.4 Release 和旧名资产仍保留，不修改历史包。

本地代码与测试包已完成；未安装新版、未执行旧安装升级，未发布新 Release。
