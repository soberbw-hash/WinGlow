# 字体重做研究记录

2026-10-03。此文件区分看见的演示、上游技术说明和本版本已实现的行为。

## 用户提供的视频

两个本地 MP4 均已提取画面检查。SF Pro + 苹方视频约 80.5 秒，vivoSans 视频约 99.2 秒。此次依据画面和字幕研究，未做音频转写；视频也没有展示改版字体内部的完整参数。

vivoSans 视频的关键步骤：

- 5–9 秒：解压工具和字体资源。
- 11–17 秒：打开 Font Replace Utility，并确认提示。
- 18–24 秒：把字体文件拖入工具；资源包含 SegUIVar.ttf、多个 Segoe UI 字重、msyh 系列 TTC。
- 25–27 秒：提示没有同名字体时忽略。
- 28–32 秒：执行任务，确认结束其它进程，并重启。
- 35–84 秒：比较日历、开始菜单、设置和资源管理器的显示。

SF Pro + 苹方视频主要展示桌面、时间、资源管理器、开始菜单和浏览器的最终效果，片尾指向 Polaris 北极星。没有看到字体制作、hinting、字形合并或垂直度量的过程。

判断：vivoSans 演示是用预先适配 Windows 字体文件名的资源执行文件替换。它的覆盖范围可以超过 FontSubstitutes。仅凭这些画面，无法证明“换一种渲染参数”就能复刻效果，更无法确认某个改版字体包的内部参数。

## 查阅的原始技术资料

1. [Microsoft 字体回退、链接与替换](https://learn.microsoft.com/en-us/globalization/fonts-layout/fonts)：GDI FontLink 用于补缺字；不同渲染器的字体选择规则不同。不能把 FontLink 当成覆盖所有应用的全局字体引擎。
2. [Microsoft DirectWrite](https://learn.microsoft.com/en-us/windows/win32/directwrite/introducing-directwrite)：渲染、字体选择和 UI 所用框架共同决定实际效果。
3. [MacType 的 SFPro-PingFang 配置作者说明](https://github.com/fy-meng/mactype-sfpro-pingfang)：作者为小字选择 Text 版本，同时明确列出回退、方框及应用兼容问题。这说明屏幕观感不能仅由一个字体名称决定。
4. [PingFang-SF 合并字体作者的实现说明](https://github.com/Juwan-Hwang/PingFang-SF)：可查看合并字体与 UPEM 统一的思路；本软件没有复制其字体文件或宣称实现其完整适配。
5. [FontMod 作者文档](https://github.com/ysc3839/FontMod)：通过 hook 修改 Win32 程序字体，不同绘制 API 有不同支持边界。
6. [Apple 字体与许可](https://developer.apple.com/fonts/)：官方 SF 字体授权有用途与再分发限制。下载链接存在，不代表可将第三方改版包放进公开安装包。
7. [Adobe 思源黑体项目](https://github.com/adobe-fonts/source-han-sans)：保留原始字体与 OFL 文件；本版本使用仓库现有 CN 资源。

## 本版本吸收的改进

- 保留三套选择：HarmonyOS Sans、思源黑体、苹方。选择思源黑体是为了提供中性的测试参照，审美由用户实测决定。
- HarmonyOS 和思源黑体使用原始 Regular 400、Medium 500、Bold 700；3.1.3 本地版新增 Windows 适配版苹方 Regular 400、Medium 500、Semibold 600。常规文字不再全部映射为 Medium。
- 三套均内置。苹方读取内部名称和真实字重，使用实际的 Semibold 完整名称映射粗体，不虚构不存在的 PingFang SC Bold。
- 遍历 TTC/OTC 集合中的字面，只安装所选简体族，不按文件名猜字体。
- 检查常用中文、标点、英文和数字的字形，以及基本行高度量。此检查不能证明所有罕见字都存在，也不能证明任意应用不会裁切文字。
- 内置字体不依赖运行时下载。资源按内容哈希缓存并回读校验。
- 保留 Emoji、Symbol 和 MDL2 图标字体，给目标字体增加缺字回退；不建立自引用链接。
- 保留当前 ClearType 与 WPF 参数，不以分辨率推测面板子像素布局，不把固定灰度参数描述为万能优化。
- 普通权限启动，具体写入时才提权。逐值备份、逐值校验、失败回退，崩溃后可恢复。
- 3.1.2 起，字体应用和普通还原成功后自动重启当前桌面的资源管理器；不自动注销或重启电脑。资源管理器文件窗口可能关闭。

## 明确边界与下一轮实测

本版本采用 Windows 字体注册与映射方式，没有实施视频中的系统文件覆盖，也没有注入应用或引入 MacType。硬编码字体、DirectWrite/UWP/WinUI 的部分界面可能不响应这些映射。不能承诺与视频相同的全桌面覆盖率。

3.1.3 本地版按用户明确要求内置 ACT-02/PingFang-for-Windows 的静态 SC 字体，保留来源、固定提交、哈希及内嵌版权声明，不将下载链接描述成开放字体许可。字体解析、字重、字形和映射由自动检查验证，真实安装与显示仍需实机检查。第三方把内部名称改成 Microsoft YaHei 的替换包仍会被拒绝，以免覆盖系统字体注册信息。

真实 Windows 实测需覆盖 100%、125%、150%、200% 缩放，特别观察资源管理器小字、开始菜单、时间冒号、设置页、中文粗体、括号、1Il/0O、Emoji 和生僻字。普通映射验证通过后，如用户仍需要视频那样的覆盖范围，再独立设计系统文件替换模式及其备份、恢复和系统更新兼容策略。

## 3.1.5：字体变化不明显与 Breeze 的区别

2026-10-03 对开发电脑进行了只读 GDI 查询。虽然全部管理映射识别为 harmonyos-sc，CreateFontW 请求 Segoe UI 或 Microsoft YaHei UI 时，GetTextFaceW 仍分别返回原字体；直接请求 HarmonyOS Sans SC 能找到该族。SPI_GETNONCLIENTMETRICS 与 SPI_GETICONTITLELOGFONT 的六个角色仍是 Microsoft YaHei UI、400，系统 DPI 为 144。结论限于这些查询：名称映射成功不足以证明真实界面正在使用目标字体，用户观察到变化不明显有具体技术依据。

华为的 [HarmonyOS Sans 官方资源页](https://developer.huawei.com/consumer/cn/design/resource-V1/)提供字体包。本版本继续使用已有内置文件，解析实际 Regular 400、Medium 500、Bold 700，用户界面只暴露鸿蒙“标准 / 粗”两档；没有修改原字体轮廓或合成粗体。“更换前”改读原生对话框当前字体与字重，是经典界面的参照，不能代表每一个应用。

审阅固定 Breeze 0.1.34 提交 bec0b6c137f5603bceaf4df7b92200cc90f663da：

- [config.cc](https://github.com/std-microblock/breeze-shell/blob/bec0b6c137f5603bceaf4df7b92200cc90f663da/src/shell/config.cc) 默认主字体直接读取 Windows/Fonts/segoeui.ttf，中文回退直接读取 msyh.ttc，再向 NanoVG 注册字体套件。
- [menu_widget.cc](https://github.com/std-microblock/breeze-shell/blob/bec0b6c137f5603bceaf4df7b92200cc90f663da/src/shell/contextmenu/menu_widget.cc) 用自己的 fontFace、fontSize、文字位置和间距绘制菜单；不是只替换系统字体名称。此机上游配置没有显式指定上述字体路径，因此按代码推断使用默认资源；没有抓取或注入运行中的菜单来验证实际选中哪个回退字面。
- 因此漂亮观感同时涉及自绘布局、文字栅格化与文件选择，不能把该菜单的效果归因于“鸿蒙已全局替换成功”，也不能通过单独设置一个字重参数把相同渲染器扩展到所有应用。

| 方式 | 能改善的范围与限制 | 本版本取舍 |
| --- | --- | --- |
| 名称映射 | 兼容部分字体选择路径，实际 GDI/DirectWrite 需验证 | 保留作补充，减少把映射当成效果成功的误判 |
| Windows 原生界面设置 | [SystemParametersInfoW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow) 的 NONCLIENTMETRICS 与图标文字 LOGFONT，针对采用系统角色字体的经典界面 | 新增，与持久字体值一起逐值备份、校验及恢复；不改字号、尺寸或 ClearType |
| Breeze 自绘 | 对右键菜单可以直接控制字体文件、字号、布局与栅格化；覆盖该菜单范围 | 继续使用独立上游引擎；本版本不接管用户的 Breeze 字体配置 |
| [FontMod](https://github.com/ysc3839/FontMod) / [MacType](https://github.com/snowie2000/mactype) | 修改程序绘制过程，API/架构/应用兼容性不同，需要注入与进程生命周期管理 | 作为后续独立渲染模块研究，未静默开启全局注入，也未复制实现 |
| 适配字体文件替换 | 视频中可提高部分硬编码字体的覆盖，但依赖字体适配、系统更新与系统文件保护 | 仍需单独工程及完整恢复验证；本版未覆盖 Windows 系统字体文件 |

原生字体设置改善的是字体选择路径，不代表 NanoVG 渲染器已进入 Windows 所有界面。新版实际应用、还原与各 UI 框架的显示仍需要实机测试。
