"""Explicit real-desktop acceptance: menu toggles/compaction, no shell restart."""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import subprocess
import threading
import time
import winreg


class Process(c.Structure):
    _fields_ = [("size", w.DWORD), ("usage", w.DWORD), ("pid", w.DWORD),
                ("heap", c.c_size_t), ("module", w.DWORD), ("threads", w.DWORD),
                ("parent", w.DWORD), ("priority", w.LONG), ("flags", w.DWORD),
                ("exe", w.WCHAR * 260)]


k = c.WinDLL("kernel32", use_last_error=True)
u = c.WinDLL("user32", use_last_error=True)
k.CreateToolhelp32Snapshot.argtypes = [w.DWORD, w.DWORD]
k.CreateToolhelp32Snapshot.restype = w.HANDLE
k.Process32FirstW.argtypes = k.Process32NextW.argtypes = [w.HANDLE, c.POINTER(Process)]
k.CloseHandle.argtypes = [w.HANDLE]
u.GetShellWindow.restype = w.HWND
u.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]


def runtimes():
    pid = w.DWORD()
    u.GetWindowThreadProcessId(u.GetShellWindow(), c.byref(pid))
    snapshot = k.CreateToolhelp32Snapshot(2, 0)
    if snapshot == c.c_void_p(-1).value:
        raise c.WinError(c.get_last_error())
    entry = Process()
    entry.size = c.sizeof(entry)
    lively, players = [], []
    try:
        more = k.Process32FirstW(snapshot, c.byref(entry))
        while more:
            if entry.exe.lower() == "lively.exe":
                lively.append(entry.pid)
            elif entry.exe.lower().startswith("lively.player."):
                players.append(entry.pid)
            more = k.Process32NextW(snapshot, c.byref(entry))
    finally:
        k.CloseHandle(snapshot)
    return {"shell": pid.value, "lively": sorted(lively), "players": sorted(players)}


def state(exe):
    root = Path(os.environ["LOCALAPPDATA"])
    result = {}
    for name, file in {
        "wallpaperLayout": root / "Lively Wallpaper/WallpaperLayout.json",
        "playlist": root / "Lively Wallpaper/WinGlowCarousel.json",
        "taskbar": root / "WindowsFontTuner/TranslucentTB/2026.2/settings.json",
        "windowMaterial": root / "WindowsFontTuner/window-material.json",
        "startMenu": root / "WindowsFontTuner/Windhawk/1.7.3/AppData/Engine/Mods/windows-11-start-menu-styler.ini",
    }.items():
        result[name] = hashlib.sha256(file.read_bytes()).hexdigest() if file.exists() else None
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Run") as key:
        result["startup"] = {}
        for name in ["WinGlow-Lively", "WinGlow-TranslucentTB", "WinGlow-StartMenu", "WinGlow-WindowMaterial"]:
            try:
                result["startup"][name] = winreg.QueryValueEx(key, name)
            except FileNotFoundError:
                result["startup"][name] = None
    font = subprocess.run([str(exe), "--diagnose-ui-fonts"], capture_output=True,
                          text=True, encoding="utf-8", timeout=30, check=True)
    result["fonts"] = json.loads(font.stdout)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", required=True, type=Path)
    parser.add_argument("--toggle-exe", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    before = state(args.exe)
    initial = runtimes()
    assert initial["shell"] and initial["lively"] and initial["players"], initial
    samples, errors, results = [], [], []
    done = threading.Event()

    def monitor():
        while not done.is_set():
            try:
                samples.append({"time": time.monotonic(), **runtimes()})
            except Exception as error:
                errors.append(str(error))
            done.wait(0.1)

    thread = threading.Thread(target=monitor)
    thread.start()
    try:
        if args.toggle_exe:
            result = subprocess.run([str(args.toggle_exe), "--verify-menu-toggle"], capture_output=True,
                                    text=True, encoding="utf-8", timeout=60, check=True)
            report = json.loads(result.stdout)
            assert report["success"], report
            results.append(report)
        for _ in range(3):
            subprocess.run([str(args.exe), "--compact-menu"], capture_output=True, timeout=60, check=True)
            path = Path(os.environ["LOCALAPPDATA"]) / "WindowsFontTuner/menu-compact-result.json"
            report = json.loads(path.read_text(encoding="utf-8"))
            assert report["success"] and "资源管理器已刷新" not in report.get("message", ""), report
            results.append(report)
            time.sleep(0.6)
        time.sleep(2)
        after = state(args.exe)
    finally:
        done.set()
        thread.join()
    unchanged = before == after
    runtime_unchanged = all({k: row[k] for k in initial} == initial for row in samples)
    report = {"success": unchanged and runtime_unchanged and not errors,
              "settingsUnchanged": unchanged, "runtimeUnchanged": runtime_unchanged,
              "initial": initial, "before": before, "after": after,
              "operations": results, "samples": samples, "monitorErrors": errors}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in report.items() if k not in ["before", "after", "samples"]}, ensure_ascii=False))
    assert report["success"], "Desktop, player, font, or unrelated settings changed"


if __name__ == "__main__":
    main()
