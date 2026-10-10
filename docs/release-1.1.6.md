# WinGlow 1.1.6

修复反复优化、撤销和手动调节后的组件识别及资源管理器刷新问题。

- Breeze 根据正在运行的程序完整路径判断来源，不再根据自启开关判断是否属于 WinGlow；复用本工具实例，退出时等待事件释放。手动开启时不再重复启动实例。其他来源的实例仍保留，不强制接管。
- 刷新前让已运行的 Lively 正常退出，确认退出后才刷新 Explorer；桌面稳定后重新启动原来的程序，由 Lively 恢复原来的壁纸布局。官方命令未完成退出时进行有限重试，仍失败则保留桌面，不强制结束主程序。修改前保存配置备份，并记录待恢复入口，应用下次启动可继续恢复。
- 修复 Windows 路径分隔符、大小写和长路径前缀造成的组件误识别；只检查当前登录会话的组件。
- 管理员授权取消且尚无核心快照时，只清理未执行的优化记录，不刷新 Explorer。
- 透明任务栏、窗口背景、开始菜单美化重新出现在“基础美化”；首页保留“任务栏、窗口与开始菜单设置”入口。

首页一键优化包括：鸿蒙粗体、Breeze 右键美化、右键一键精简、快捷方式箭头与盾牌角标隐藏、模糊任务栏与细线、窗口背景、开始菜单美化（20% 背景浓度、圆角 10；已有自定义参数保留）。Windows 不支持的效果不强行应用，外部 Windhawk 冲突会明确报告。动态壁纸与经典布局继续单独可选，不自动安装。

验证：79 项常规 Rust 测试通过，严格 Clippy、TypeScript/Vite 和菜单过滤检查通过；八组首页居中与两处设置入口检查通过。实机 Breeze 来源检查通过；30 秒内连续两次 Explorer 刷新时，Lively 保持运行、壁纸布局文件不变；另一轮连续刷新确认任务栏组件未弹保护错误、配置不变，Breeze 保持运行。当前保存的动态壁纸列表为空，尚未验证具体视频或网页壁纸播放恢复；没有将连续刷新测试等同于完整一键优化/撤销的实机验收。

控制及恢复逻辑参考上游固定版本源码：[Breeze 生命周期](https://github.com/std-microblock/breeze-shell/blob/0.1.34/src/inject/inject.cc)、[Lively 控制入口](https://github.com/rocksdanister/lively/blob/v2.2.1.0/src/Lively/Lively/App.xaml.cs)、[Lively 壁纸恢复](https://github.com/rocksdanister/lively/blob/v2.2.1.0/src/Lively/Lively/Core/WinDesktopCore.cs)。
