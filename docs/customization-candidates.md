# Windows 美化工具候选

用户指定四项的进一步判断见 [视觉扩展集成方案](visual-integration-proposal.md)。仅调研，未改变软件集成状态。

调研日期：2026-10-03。以下只供用户浏览和选择，均未额外安装或集成。用途依据各项目官方仓库；集成顺序为 WinGlow 产品判断，不是下载量排名。

| 项目 | 用户能得到什么 | 适合怎么放进 WinGlow |
| --- | --- | --- |
| [Windhawk](https://github.com/ramensoftware/windhawk) | 任务栏、开始菜单和程序界面的定制模块 | 先筛选少量明确主题，再考虑独立开关；模块会修改运行中的程序，需要按 Windows 版本验证 |
| [Lively Wallpaper](https://github.com/lively-community/lively) | 视频、网页和互动动态壁纸 | 单独“壁纸”类别，支持运行状态、暂停和原壁纸恢复；引擎和媒体比当前美化选项更重 |
| [Rainmeter](https://github.com/rainmeter/rainmeter) | 桌面时钟、天气、资源监控和皮肤 | 单独“桌面组件”，可先提供几套清楚的模板；不要默认安装大量皮肤 |
| [ExplorerPatcher](https://github.com/valinet/ExplorerPatcher) | 任务栏、资源管理器和 Windows 外壳行为调整 | 独立高级选项，修改面较大；先评估与 Breeze、TranslucentTB 的兼容及恢复 |
| [Open-Shell](https://github.com/Open-Shell/Open-Shell-Menu) | 经典开始菜单与相关外观 | 仅用户明确需要经典菜单时启用，不作为默认首页方案 |
| [DWMBlurGlass](https://github.com/Maplespe/DWMBlurGlass) | 全局窗口标题栏材质与模糊效果 | 独立窗口外观实验项，需要确认 Windows 更新兼容和退出恢复 |

视觉变化优先可先看 Lively（动态壁纸）和 Rainmeter（桌面组件）；想精调 Windows 界面则看 Windhawk。这是建议，只有用户确认后才实施下一项。

已授权本轮集成的是 [TranslucentTB](https://github.com/TranslucentTB/TranslucentTB/releases/tag/2026.2)：透明任务栏，采用官方未修改 Windows 11 x64 便携版本与独立配置。已有 Breeze 菜单美化继续保留。两者不是上表候选的默认安装授权。
