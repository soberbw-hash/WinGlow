# 3.1.0 本地验证记录

2026-10-03，Windows x64。本记录对应本地测试包。

## 已验证

- Rust fmt、Clippy all-targets 严格检查通过；22 项测试通过。
- 测试包含内存后端失败注入、激活失败回退、不可变快照、不存在值还原、白名单拒绝越界、旧 REG 解析、字体字重和真实内置资源检查。
- 新增原生注册表测试仅在临时的 HKCU\Software\WindowsWeitiao\TransactionTests\UUID 下运行：还原 Unicode 字符串、DWORD 原类型及不存在状态；第二次写入故障后回退已写值。测试结束删除自己创建的测试键，不碰 Windows 字体或 Explorer 配置。
- 额外检查损坏快照显式隔离并保留原文；桌面实时文字状态在持久 FFlags 缺失时仍能进入快照。
- TypeScript/Vite、Tauri x64 NSIS 与便携包的结果见本记录末尾的打包结果。
- Playwright 浏览器检查字体选择、三类导航、还原页；前后预览的假文件信息与冗余说明已删除。截图保存在 WinGlow.App/output/playwright。

## 实机边界

只读取真实菜单与已有字体状态。未在用户电脑上开关字体、箭头、盾牌、桌面文字或菜单，未启动 Breeze，未执行 DISM/SFC，未自动注销或重启。因此不能把还原事务测试通过描述为全部系统视觉效果已实测通过。

Windows 10/11 与不同 DPI、UAC 取消、安装升级/卸载、Breeze 开关后注销、箭头缓存、实验盾牌覆盖、苹方实际导入、字体真实应用再普通/扫描还原，应在虚拟机或可恢复的测试机上完成。

## 最终结果

- TypeScript/Vite、Tauri release 与 x64 NSIS 打包通过；重新检查最终修改后仍为 22 项测试通过。npm 生产依赖审计为 0 漏洞。
- 模拟 Tauri IPC 的界面测试通过：开关应用后重新读取状态、写入拒绝时不显示为启用、菜单搜索与开关、分类还原参数正确、取消普通还原/扫描修复不发出请求。模拟不操作 Windows，也不证明实际系统效果。
- 最终 release 的只读诊断读取到 62 个菜单项、5 个开关，无未完成事务。桌面 COM 成功读取实时 flags 为 0x40200224；只读诊断前后字体和菜单 JSON 完全相同，已有思源黑体状态未更改。
- 本地安装包 WindowsWeitiao-3.1.0-Setup.exe：72,008,699 字节；SHA-256 a8d7971e244b9a83330f0822979e86bed08691786143c15a027614213124f559。
- 便携 ZIP WindowsWeitiao-3.1.0-Portable.zip：79,466,641 字节；SHA-256 0553bf58fcf400f4ec8ee6b7662c466b3ce7256993ceba0657eb7fd16658e9f1。
- 校验清单一致，便携 EXE 与最终 release EXE 字节相同，字体/Breeze 许可证与来源声明齐备；包内没有 Breeze EXE/DLL。
- 最终字体页在 1080×740 与最低 760×600 浏览器窗口下均无横向/主内容纵向溢出，HarmonyOS 粗体资源加载完成；浏览器和模拟 IPC 检查均无控制台错误。

未发布 GitHub Release。
