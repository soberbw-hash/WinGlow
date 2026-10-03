# WinGlow主线

本目录是 Rust + Tauri 2 + React 的 3.1.1 主线。使用说明见仓库 [README](../README.md)，维护约定见 [development.md](../docs/development.md)。

```powershell
npm ci
npm run tauri dev
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --bundles nsis
```

浏览器预览只展示界面，应用按钮禁用。真实字体修改需在 Windows 应用中操作，先备份再写入。系统实测不包含在浏览器验证结果中。
