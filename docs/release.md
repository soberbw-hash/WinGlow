# WinGlow 发布流程

重做版本从 1.0.0 起，旧系列首次迁移需手动安装，后续版本严格递增。

同步修改 WinGlow.App 的 package.json、package-lock.json、src-tauri/Cargo.toml 和 tauri.conf.json 应用版本。保留 identifier，以便恢复旧备份。

运行 scripts/Build-ModernPackages.ps1，执行格式、Clippy、Rust 测试、前端构建与 NSIS 打包，产出安装包、签名、便携包、latest.json 和 SHA256SUMS.txt。

更新密钥通过 TAURI_SIGNING_PRIVATE_KEY 或 -SigningKeyPath 提供。默认读取当前用户 .codex/secrets/WinGlow/updater.key。禁止提交 Git 或上传公开附件；密钥需要受访问权限保护的备份。配置中的公钥可以公开。

创建 v版本号 Release，上传上述文件并设为 latest。latest.json 必须为无 BOM UTF-8，windows-x86_64 地址指向同一标签的 WinGlow-版本号-Setup.exe。

验证公开更新清单、下载哈希、签名与安装启动，成功后才清理旧 Release 和旧构建目录。保留 Git 历史、标签和用户备份。

下载由 Tauri 验证签名及版本一致性，用户点击才安装并重新打开。系统修改互斥锁及未完成恢复状态会阻止更新安装。便携版通过同一安装包迁移到安装版。
