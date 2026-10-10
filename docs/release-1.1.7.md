# WinGlow 1.1.7

修复部分电脑正常双击打开后仍被提示“请以普通权限打开 WinGlow”的问题。

- 使用实际进程令牌比较 WinGlow 与当前桌面的完整性级别，不再因为管理员身份直接拒绝 Breeze 和透明任务栏。关闭 UAC 或使用内置 Administrator 时，只要软件与桌面属于同一账户、权限一致，就允许进入原有启动和验证流程。
- 软件被单独提权、桌面权限较低时，在主界面创建前自动使用桌面令牌重新打开同一份 WinGlow。仅适配当前账户，限制为一次重新启动，防止循环。需要管理员权限的字体和注册表修改仍由专用流程处理。
- 不修改 UAC 或其他系统安全策略。无法验证账户或权限时，在桌面美化写入前停止并说明原因；不冒充组件启动成功。
- 保留 1.1.6 的一键优化内容、自动备份和恢复、资源管理器刷新时的组件保护。

验证：81 项 Rust 测试通过（3 项显式桌面刷新测试未重复执行），严格 Clippy、TypeScript/Vite 与菜单过滤检查通过。实际程序的只读权限验收确认普通启动匹配桌面权限；管理员启动自动重新打开后，应用与桌面均为 Medium 完整性级别，账户匹配且兼容检查通过。无需更改系统安全策略。

本机没有关闭 UAC；同权限管理员桌面的分支由权限矩阵测试覆盖，你朋友电脑上的完整桌面效果仍需更新后复测。没有把权限分支验证等同于所有第三方组件在该环境下均支持渲染。

实现参考：[Windows 进程令牌信息](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-gettokeninformation)、[按指定令牌启动进程](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createprocesswithtokenw)。
