"""Encode WinGlow application assets from the unchanged user-supplied original.

Run with Python + Pillow. No drawing, crop, recoloring, or background replacement.
"""
from pathlib import Path
import hashlib
import json
import shutil
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
BRAND = ROOT / "docs" / "branding"
MASTER = BRAND / "icon-master.png"
APP = ROOT / "WinGlow.App"
ICONS = APP / "src-tauri" / "icons"

def main():
    source = Image.open(MASTER)
    if source.mode != "RGBA" or source.width != source.height:
        raise ValueError("The original must be square RGBA; preserve its transparency.")
    ICONS.mkdir(parents=True, exist_ok=True)
    sizes = {
        "icon.png": 1024, "32x32.png": 32, "128x128.png": 128,
        "128x128@2x.png": 256, "Square30x30Logo.png": 30,
        "Square44x44Logo.png": 44, "Square71x71Logo.png": 71,
        "Square89x89Logo.png": 89, "Square107x107Logo.png": 107,
        "Square142x142Logo.png": 142, "Square150x150Logo.png": 150,
        "Square284x284Logo.png": 284, "Square310x310Logo.png": 310,
        "StoreLogo.png": 50,
    }
    for name, size in sizes.items():
        source.resize((size, size), Image.Resampling.LANCZOS).save(ICONS / name)
    source.save(ICONS / "icon.ico", sizes=[(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)])
    source.resize((1024, 1024), Image.Resampling.LANCZOS).save(ICONS / "icon.icns")
    source.resize((256, 256), Image.Resampling.LANCZOS).save(APP / "src" / "assets" / "app-icon.png")
    (APP / "public").mkdir(exist_ok=True)
    source.resize((64, 64), Image.Resampling.LANCZOS).save(APP / "public" / "favicon.png")
    (ROOT / "Assets").mkdir(exist_ok=True)
    shutil.copyfile(ICONS / "icon.ico", ROOT / "Assets" / "AppIcon.ico")
    # A preserved original also accompanies the historical project resources.
    shutil.copyfile(MASTER, ROOT / "Assets" / "WinGlow-icon-master.png")
    manifest = {
        "product": "WinGlow", "source": "user-provided-original",
        "originalFilename": "codex-clipboard-6259622c-3309-4989-908a-cd8d36d4ef24.png",
        "master": "icon-master.png", "sha256": hashlib.sha256(MASTER.read_bytes()).hexdigest(),
        "width": source.width, "height": source.height, "mode": source.mode,
        "transform": "unchanged master; size and format encoding only",
        "icoSizes": [16, 20, 24, 32, 40, 48, 64, 128, 256],
    }
    (BRAND / "brand-source.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print("WinGlow assets encoded; original SHA-256:", manifest["sha256"])

if __name__ == "__main__":
    main()
