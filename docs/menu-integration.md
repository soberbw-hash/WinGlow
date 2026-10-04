# ContextMenuManager 借鉴与接入

2026-10-05。对照官方源码及用户截图研究，采用以下内容：

- 分类分层：常用位置、文件类型、其他位置；保留直接点击的细分类，避免堆积一长排。
- 完整位置识别：Folder 与 Directory 同归文件夹；补充应用快捷方式、此电脑、回收站、库和未知类型。
- 实际文件关联：读取当前 UserChoice 或默认 ProgID，补充常见图片、文本、PDF、压缩和音视频关联程序。只读取固定类型，不向用户解释注册表术语。
- GUID 字典：引入原始名称/图标数据，内置离线版本、自动更新、上次有效缓存、手动检查；网络数据不能生成命令、不能改变自动精简决策。
- 状态筛选：全部、正在显示、已隐藏；既能精简，也能快速找到需要重新开启的项目。
- 保留原有真实子菜单展开、自动备份、基本操作保护，以及只减不加的自动精简规则。

暂不搬入任意注册表编辑、永久删除、IE 菜单、Win+X 修改、任意增强命令和完整字典编辑器。这些与 WinGlow 的极简体验无直接关系，也会扩大恢复与权限范围。“新建”“发送到”“打开方式”的逐项管理涉及不同的文件/关联恢复机制，本轮不伪装成可用开关。

其“网络数据库”实际是 GitHub/Gitee 下载的 INI/XML 字典；应用内还带内置字典和用户字典。本轮仅引入只读 GUID 元数据，其他 XML 中的命令及修改规则不执行。当前上游为 GPL-3.0，导入文件的来源和许可保存在 [字典来源记录](../third-party/ContextMenuManager/SOURCE.md)。

参考：[GuidInfo](https://github.com/BluePointLilac/ContextMenuManager/blob/55507155dd8e49c7ab4606da97f2af192d590dfe/ContextMenuManager/Methods/GuidInfo.cs)、[Updater](https://github.com/BluePointLilac/ContextMenuManager/blob/55507155dd8e49c7ab4606da97f2af192d590dfe/ContextMenuManager/Methods/Updater.cs)、[ShellList 分类](https://github.com/BluePointLilac/ContextMenuManager/blob/55507155dd8e49c7ab4606da97f2af192d590dfe/ContextMenuManager/Controls/ShellList.cs)。
