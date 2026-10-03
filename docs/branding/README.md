# WinGlow 品牌资源

`icon-master.png` 是用户在 2026-10-03 提供的原图，按原始字节保存；1254×1254，RGBA，透明背景。`brand-source.json` 保存来源和 SHA-256。

直接使用原图，不重画、不改色、不裁切、不添加光效。程序与安装包需要的 PNG、ICO、ICNS 仅做尺寸/格式编码。原图和来源文件随安装版及便携版一起放在 `branding/`，便于后续取用。

重新生成所有图标：

```powershell
python scripts/generate-brand-assets.py
```

需要 Pillow。根目录的历史图标脚本和主线图标脚本均转到该脚本，不再绘制旧 Aa 图标。

界面沿用原图的冰蓝背景、青蓝重点与少量淡紫边缘色，保持极简布局，不添加营销文案或额外控制。
