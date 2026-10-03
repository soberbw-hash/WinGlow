# WinGlow 3.1.3 苹方内置验证

日期：2026-10-03。

## 本次变化

苹方与 HarmonyOS Sans、思源黑体一样随软件内置。选择苹方即可看到真实预览，再点击应用，无需另找文件、导入或联网下载。内置字体来源为 ACT-02/PingFang-for-Windows，固定提交 `8444592e20420a9d412a851542f00f7ee9c3144f`。

使用 Regular 400、Medium 500、Semibold 600 三个静态字重；粗体映射至真实的 `PingFang SC Semibold`，不构造不存在的 Bold 名称。后端通过 include_bytes 嵌入，前端 @font-face 使用同一组原始文件。文件来源、固定提交、Git blob 校验与 SHA-256 清单见 FontPackages/pingfang-sc/SOURCE.txt，字体版权声明随两种包保存。

仍保留旧版导入资源记录与字体事务备份的读取及恢复兼容。升级不会自动应用字体；应用成功后沿用 3.1.2 的资源管理器重启流程。

## 已通过的检查

- 三个字体文件与固定提交的 Git blob 完全一致；字体文件没有重命名内部名称或合并字形。
- 26 项 Rust 测试通过：三套字体的真实字重、常用中文/英文/数字/标点、基本行高度量；苹方完整名称及粗体映射；当前方案检测；事务备份、回退及还原检查。
- Rust 格式检查、严格 Clippy、TypeScript 和 Vite 生产构建通过。
- Playwright 选择苹方后三个 FontFace 均为 loaded；更换后段落实际平台字体为 `PingFang SC`、PostScript 名称 `PingFangSC-Regular`、isCustomFont=true，没有回退成默认字体。“先导入”提示不存在。
- 新 EXE 的 `--diagnose` 正常返回，三套候选均 available=true；开发电脑原有思源黑体方案仍可识别，canRestore=true，无待恢复事务。
- 安装包和便携包生成到 artifacts/WinGlow-3.1.3，附 SHA256SUMS.txt。尚未发布 GitHub Release。
- 两个软件包的 SHA-256 与清单一致；便携包包含苹方来源和版权声明、官方图标原图，包内 EXE 与通过只读诊断的构建完全一致。

浏览器检查截图：WinGlow.App/output/playwright/pingfang-preview-3.1.3.png（本地检查文件，不提交仓库）。

## 验证边界

未执行安装程序，未实际更换开发电脑字体，未重启开发电脑的资源管理器。浏览器预览和映射测试不能替代 Windows 各应用中的实际效果检查。

下一次实机检查：安装或运行新版，选择苹方并应用，确认桌面恢复，检查资源管理器小字、中英文、数字、粗体与 Emoji；随后普通还原并核对原方案恢复。扫描修复只在普通还原失效时独立验证。
