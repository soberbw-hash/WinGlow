# WinGlow 1.1.12 本地验收

日期：2026-10-11。本次仅更新本机测试版本，未推送 GitHub、上传更新元数据或发布公网版本。

## 本轮修复

- Lively 播放器将本地目录映射到 WebView2 虚拟主机；原代码把 Rust canonicalize 的 verbatim 路径传给播放器，导致相对 CSS/脚本/图像加载失败，出现白底裸文本。现在先核对项目及入口文件边界，再转换为标准 Windows 路径，同时修正缩略图路径匹配。
- 轮播现在由独立 WinGlow --wallpaper-host 切换真实壁纸库，默认5分钟。手动点击停止轮播并立即应用，关闭主界面后轮播继续。启动项指向已安装 WinGlow；快速恢复通过新 generation 让存活的宿主重置选择及计时，避免恢复后的误停止。
- 删除壁纸音量、暂停、继续、静音、停止及额外应用按钮。彻底移除经典布局的 UI、IPC、安装下载与设置实现；保留旧字体事务的兼容范围及外部任务栏冲突检测。未删除用户自行安装的第三方工具。
- 基础任务栏编辑同时覆盖桌面、普通窗口、搜索及任务视图场景，最大化与开始菜单允许单独效果，避免普通窗口仍使用旧配置。
- 字体手动切换也修正旧的小字号，最低9pt按实际 DPI 换算，保留更大字号；启动与桌面重建核对实际桌面缓存。版本迁移只向新版本进行，不让旧客户端再次套用更旧方案。

## 自动验证与真实设备验证

- Rust 格式检查、严格 Clippy 通过；104项单元测试：100通过、4项显式设备写入/重启测试跳过、0失败。TypeScript及生产构建通过。
- 浏览器 IPC fixture 在1024×720、1707×960、2560×1440、3840×2160核对单个主滚动区，无横向溢出/根页面空白/嵌套滚动；经典布局及壁纸控制行消失，图片点击直接发送选择请求。浏览器 fixture 不等于原生播放器渲染验收。
- 本机3840×2160、150%缩放：实际应用思源黑体后为 -18/400；再应用鸿蒙粗体为 -18/700。六个 SPI 字体角色和真实桌面 WM_GETFONT 相符。这些字体切换验收未重启 Explorer。
- 12个库项目逐张真实切换并截图：Fluids、Simple System、Rain、Triangles & Light、The Hill、Living Room、Medusae、Periodic Table、Parallax.js、Music Tunnel、Matrix Rain Customizable、Music TV。均显示有样式的画面，非白底裸文本。现场截图位于 Codex 临时目录 winglow-wallpaper-1112-check，含 contact-sheet.png。
- 任务栏实际应用不透明蓝色到六个场景后回读一致，最后配置及启动项精确恢复。开始菜单实际 Styler DLL 已加载，自定义浓度35/圆角22到达配置；窗口材质实测0→2→3→0，后台应用/暂停/恢复有效，原配置精确恢复。
- 签名安装器与可信签名注释验签通过，4项产物 SHA256匹配。静默安装退出0，安装版本1.1.12；安装与便携主程序在归一化 Tauri bundle marker 后逐字节一致。桌面与开始菜单快捷方式指向本次 EXE 的真实文件路径及同目录独立 ICO。Codex 启动链触发 Windows 应用目录虚拟化，真实安装位置为 LOCALAPPDATA/Packages/OpenAI.Codex_2p2nqsd0c76g0/LocalCache/Local/WinGlow；已明确解析文件路径，避免快捷方式在 Explorer 中指向另一个旧 AppData 安装。
- 安装版执行一次完整 --apply-one-click，返回 success=true，字体/菜单/图标/效果和真实轮播完成；实际桌面仍为鸿蒙粗体 -18/700，DPI144；轮播包含12项，interval_seconds=300。Explorer开始16240、结束28676。完整优化采样脚本在结束读取 UTF-8 结果时发生编码错误，时间采样未保存；因此没有保留逐时PID证据，也没有再次重复桌面刷新测试。集中刷新调用次数另有代码与单元测试覆盖。
- 已打开安装版主界面，首页显示已优化，无旧折叠区域或错误条。GUI及窗口材质/轮播宿主均来自安装目录的1.1.12，无旧便携 GUI。

- 安装版独立轮播宿主额外完成有界计时验收：Rain→Fluids→Simple System→Triangles & Light→The Hill，真实 WallpaperLayout 路径依次改变；测试短间隔已恢复为300秒，并更新 generation 重置计时。Explorer仍为28676，没有因轮播切换而重启。
- 最后桌面截图确认图标文字明显恢复，实际字体再次回读 -18，GUI首页无折叠区域。

## 验收边界

尚未重启/注销验证登录后字体持久性，未改 UAC、显示缩放或分辨率。DPI计算覆盖100%–300%，不代表多台不同显示器都完成实机验收。窗口材质影响支持窗口的系统背景与标题栏，不会把不透明应用内容改成磨砂。语音包音量请求属于上号，按用户确认忽略。

参考：[固定 Lively 播放器本地虚拟主机映射](https://github.com/lively-community/lively/blob/v2.2.1.0/src/Lively/Lively.Player.WebView2/Extensions/WebView2/CoreWebView2Extensions.cs)。
