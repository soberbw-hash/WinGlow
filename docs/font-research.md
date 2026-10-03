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
- 内置方案使用原始 Regular 400、Medium 500、Bold 700，常规文字不再全部映射为 Medium。
- 苹方由用户导入有权使用的静态 SC 字体；读取内部名称和真实字重，要求常规与粗体齐备。
- 遍历 TTC/OTC 集合中的字面，只安装所选简体族，不按文件名猜字体。
- 检查常用中文、标点、英文和数字的字形，以及基本行高度量。此检查不能证明所有罕见字都存在，也不能证明任意应用不会裁切文字。
- 内置字体不依赖运行时下载。资源按内容哈希缓存并回读校验。
- 保留 Emoji、Symbol 和 MDL2 图标字体，给目标字体增加缺字回退；不建立自引用链接。
- 保留当前 ClearType 与 WPF 参数，不以分辨率推测面板子像素布局，不把固定灰度参数描述为万能优化。
- 普通权限启动，具体写入时才提权。逐值备份、逐值校验、失败回退，崩溃后可恢复。
- 不结束用户程序，不自动重启资源管理器、不自动注销。

## 明确边界与下一轮实测

本版本采用 Windows 字体注册与映射方式，没有实施视频中的系统文件覆盖，也没有注入应用或引入 MacType。硬编码字体、DirectWrite/UWP/WinUI 的部分界面可能不响应这些映射。不能承诺与视频相同的全桌面覆盖率。

苹方安装包不内置字体。没有取得合法测试字体前，苹方真实安装、字重和显示不能算验证完成。第三方把内部名称改成 Microsoft YaHei 的替换包会被拒绝，以免覆盖系统字体注册信息；请使用内部字体族名为 PingFang SC 的静态版本。

真实 Windows 实测需覆盖 100%、125%、150%、200% 缩放，特别观察资源管理器小字、开始菜单、时间冒号、设置页、中文粗体、括号、1Il/0O、Emoji 和生僻字。普通映射验证通过后，如用户仍需要视频那样的覆盖范围，再独立设计系统文件替换模式及其备份、恢复和系统更新兼容策略。
