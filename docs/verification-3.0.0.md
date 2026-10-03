# 3.0.0 本地验证记录

2026-10-03，Windows x64。此记录对应本地测试版，不代表 GitHub 发布或真实字体应用已验证。

- TypeScript 编译、Vite 生产构建、Tauri x64/NSIS 打包通过。
- Rust fmt、Clippy `--all-targets -- -D warnings` 通过。MSVC 链接器输出创建库/对象的信息，未出现编译错误。
- 15 项 Rust 测试通过：逐值备份/回读、失败回退、不存在值恢复、字重映射、真实内置字体解析、旧 REG 解析、v2/v3 重叠恢复值去重等。
- npm audit（含开发依赖）报告 0 漏洞。
- Playwright 浏览器验证：三项字体切换、原生 radio 键盘选择、功能导航和恢复页；没有浏览器控制台错误。HarmonyOS/思源的常规与粗体资源实际载入。
- 1080×740 字体页无横向或主内容纵向溢出；760×600 使用纵向滚动查看，不作为真实 DPI 测试。
- release EXE `--diagnose` 成功读取本机 Windows 默认映射，返回三套候选，苹方未导入。检查前后 FontSubstitutes 输出相同。
- 生成安装包、带来源/许可证的便携包和 SHA-256 清单。

未执行：真实字体导入/应用/恢复、UAC 取消路径、注销重启后的显示、不同 Windows 版本与实际 DPI、苹方真实字体、安装器安装升级及卸载。未修改开发电脑的字体映射，未发布 GitHub Release。
