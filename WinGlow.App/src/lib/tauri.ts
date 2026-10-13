import { invoke, isTauri } from "@tauri-apps/api/core";
import type { ActionResult, BootstrapPayload, ShellState, RepairProgress, Appearance, DesktopSettings, TaskbarSettings } from "../types";
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
    { id: "lively", label: "动态壁纸", installed: false, enabled: false, managed: false, supported: true, note: "选择动态壁纸；默认静音，全屏和电池模式暂停。" },
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
export const restoreNormal = () => invoke<ActionResult>("restore_normal");
export const optimizeMenu = () => invoke<ActionResult>("optimize_menu");
export const openBackup = () => invoke<void>("open_backup");
export const optionalTool = (id: string, action: "install" | "open" | "remove" | "enable" | "disable" | "system-taskbar") => invoke<ActionResult>("optional_tool", { id, action });

export const chooseWallpaper = () => invoke<ActionResult>("choose_wallpaper");
export const optimizationUpgradeStatus = () => invoke<{running:boolean;finished:boolean;message:string;error:boolean}>("optimization_upgrade_status");
export const retryOptimizationUpgrade = () => invoke<ActionResult>("retry_optimization_upgrade");
export const wallpaperControl = (action: "pause" | "play" | "mute" | "stop" | "restore-settings") => invoke<ActionResult>("wallpaper_control", { action });
export async function loadDesktopSettings():Promise<DesktopSettings> {
  if(isTauri()) return invoke("load_desktop_settings");
  return {taskbar:{desktop:{accent:"blur",color:"#000000",opacity:0,showLine:true},maximized:null,startOpen:null},wallpaper:{items:[{id:"builtin:night",title:"夜色",preview:null,scene:"night"},{id:"builtin:sea",title:"海色",preview:null,scene:"sea"},{id:"builtin:dawn",title:"晨光",preview:null,scene:"dawn"}],selected:null,intervalMinutes:5,volume:0,canRestore:false}};
}
export const applyTaskbarSettings = (settings:TaskbarSettings)=>invoke<ActionResult>("apply_taskbar_settings",{settings});
export const selectWallpaper = (id:string,minutes:number)=>invoke<ActionResult>("select_wallpaper",{id,minutes});
export const setWallpaperVolume = (volume:number)=>invoke<ActionResult>("set_wallpaper_volume",{volume});

export const openHomepage = (kind: "author" | "project") => invoke<void>("open_homepage", { kind });
