import { invoke, isTauri } from "@tauri-apps/api/core";
import type { ActionResult, BootstrapPayload, ShellState, RepairProgress, Appearance } from "../types";
// Browser previews cannot write Windows settings or report a simulated success.
export async function loadBootstrap(): Promise<BootstrapPayload> {
  if (isTauri()) return invoke("load_bootstrap");
  return {
    presets: [
      { id: "harmonyos-sc", label: "HarmonyOS Sans", previewFamily: '"HarmonyOS Preview", "Microsoft YaHei UI", sans-serif', available: true },
      { id: "harmonyos-sc-bold", label: "HarmonyOS Sans · 粗", previewFamily: '"HarmonyOS Bold Preview", "Microsoft YaHei UI", sans-serif', available: true },
      { id: "source-han-sans-cn", label: "思源黑体", previewFamily: '"Source Han Preview", "Microsoft YaHei UI", sans-serif', available: true },
      { id: "pingfang-sc", label: "苹方", previewFamily: '"PingFang Preview", "Microsoft YaHei UI", sans-serif', available: true },
    ],
    activePresetId: null, activeFontLabel: "Windows 默认",
    currentPreviewFamily: '"Segoe UI", "Microsoft YaHei UI", sans-serif',
    currentPreviewWeight: 400,
    needsFontRefresh: false,
    backupDir: "", canRestore: false, pendingRecovery: null,
  };
}
export const applyFont = (presetId: string, paths: string[] = []) => invoke<ActionResult>("apply_font", { presetId, paths });
export const restoreFonts = (mode: "last" | "default") => invoke<ActionResult>("restore_fonts", { mode });
export const importFonts = (paths: string[]) => invoke<ActionResult>("import_fonts", { paths });
export async function loadShell(): Promise<ShellState> {
  if (isTauri()) return invoke("load_shell");
  return { items: [], breezeEnabled: false, taskbarEnabled: false, taskbarSupported: true, startMenuEnabled: false, startMenuSupported: true, windowMaterialEnabled: false, windowMaterialSupported: true, appearance: { material: 3, tint: 20, radius: 10 }, optionalTools: [
    { id: "lively", label: "动态壁纸", installed: false, managed: false, supported: true, note: "选择动态壁纸；默认静音，全屏和电池模式暂停。" },
    { id: "explorer-patcher", label: "经典布局", installed: false, managed: false, supported: true, note: "ExplorerPatcher：可调整任务栏和开始菜单；不会随一键优化安装。" },
  ], optimizationActive: false, optimizationPending: false, pendingRecovery: false, tweaks: [
    { id: "shortcut-arrow", label: "隐藏快捷方式箭头", enabled: false, note: null },
    { id: "shield-overlay", label: "隐藏盾牌角标", enabled: false, note: "实验功能，部分 Windows 版本可能不生效。" },
  ] };
}
export const setTweak = (id: string, enabled: boolean) => invoke<ActionResult>("set_tweak", { id, enabled });
export const setAppearance = (id: string, settings: Appearance) => invoke<ActionResult>("set_appearance", { id, settings });
export const setMenuItem = (id: string, enabled: boolean) => invoke<ActionResult>("set_menu_item", { id, enabled });
export const restoreCategory = (category: "menu" | "details" | "all-last") => invoke<ActionResult>("restore_category", { category });
export const recoverPending = () => invoke<ActionResult>("recover_pending");
export const repairSystem = () => invoke<ActionResult>("repair_system");
export const repairProgress = () => invoke<RepairProgress>("repair_progress");
export const optimizeSystem = (restore = false) => invoke<ActionResult>("optimize_system", { restore });
export const optimizeMenu = () => invoke<ActionResult>("optimize_menu");
export const openBackup = () => invoke<void>("open_backup");
export const optionalTool = (id: string, action: "install" | "open" | "remove" | "system-taskbar") => invoke<ActionResult>("optional_tool", { id, action });
export const refreshMenuDictionary = () => invoke<ActionResult>("refresh_menu_dictionary");

export const chooseWallpaper = () => invoke<ActionResult>("choose_wallpaper");
export const wallpaperControl = (action: "pause" | "play" | "mute" | "stop") => invoke<ActionResult>("wallpaper_control", { action });
