# WinGlow 视觉扩展集成判断

原始调研日期：2026-10-03。以下保留初始判断；用户于 2026-10-04 授权窗口磨砂、开始菜单美化加入一键优化，动态壁纸与 ExplorerPatcher 单独可选。**当前实现和验证边界以 [1.1.0 集成记录](visual-integrations.md) 为准**，下表不再代表当前完成状态。

| 项目 | 判断 | 接入方式 |
| --- | --- | --- |
| DWMBlurGlass | 下一项窗口外观候选 | “窗口磨砂”开关及一套清晰预设；验证 Windows build 与符号；按官方安装、卸载机制管理。不能仅关闭 GUI 就认为效果卸载。 |
| Windhawk | 后续精选功能底座 | 先验证开始菜单样式及局部字体，限制为少量模块；分别固定引擎、模块版本。不把整个模块市场放进 WinGlow。 |
| Lively | 独立可选动态壁纸 | 按需准备引擎，利用官方命令控制选择、暂停、音量和关闭；默认静音、全屏/电池暂停。 |
| ExplorerPatcher | 暂缓直接集成 | 安装会提升权限、刷新 Explorer，并改变任务栏样式；更适合明确的经典外壳需求。 |

这些不是系统字体替换引擎，不替换现有字体和恢复核心。Windhawk 的 Start Menu Styler 支持 FontFamily/FontWeight 样式，但鸿蒙字体在具体目标控件上的效果需要验证，不能承诺改变所有现代应用字体。

## 接入规则

- 现有一键优化内容保持不变，新功能先独立验证后由用户确定是否加入。
- 同一外观属性只由一个工具管理：TranslucentTB 已负责任务栏材质，Windhawk 第一阶段优先开始菜单；未来任务栏方案切换必须保留原配置。不是认定所有工具必然冲突，兼容需实测。
- 每次操作前备份原配置精确字节、原运行状态、组件版本和启动入口；失败自动撤销，只恢复 WinGlow 本次改变，不清空用户自己的模块和壁纸库。
- 区分用户已经安装的实例与 WinGlow 管理的实例；不结束其他来源组件，不卸载用户既有工具。
- DWMBlurGlass 官方提醒 Windows 更新后可能需要重新下载符号；具体发行版的自动安装/卸载接口尚需验证，不臆造命令参数。
- Windhawk 的模块安装、设置和启停自动化接口尚需验证；保留上游安全模式作为独立恢复路径。
- Lively 支持官方命令控制，但每屏原壁纸、布局、既有运行状态及 Explorer 刷新后的恢复仍需适配；不默认打包大量媒体。
- 固定官方版本和哈希、保留许可及来源，组件独立更新和回退；验证启停、失败恢复、系统更新、连续 Explorer 刷新、多屏和 2K/4K 缩放后再扩大范围。

## 官方依据

- [DWMBlurGlass](https://github.com/Maplespe/DWMBlurGlass)
- [Windhawk](https://github.com/ramensoftware/windhawk)
- [Start Menu Styler 源码](https://github.com/ramensoftware/windhawk-mods/blob/main/mods/windows-11-start-menu-styler.wh.cpp)
- [Windhawk 安全模式](https://ramensoftware.com/windhawk-v1-3-beta)
- [Lively](https://github.com/lively-community/lively)
- [Lively 命令控制](https://github.com/lively-community/lively/wiki/Command-Line-Controls)
- [ExplorerPatcher 安装与卸载](https://github.com/valinet/ExplorerPatcher)
