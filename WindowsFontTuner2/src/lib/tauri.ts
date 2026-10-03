import { invoke, isTauri } from "@tauri-apps/api/core";
import type { ActionResult, BootstrapPayload } from "../types";
// Browser previews cannot write Windows settings or report a simulated success.
export async function loadBootstrap(): Promise<BootstrapPayload> {
  if (isTauri()) return invoke("load_bootstrap");
  return {
    presets: [
      { id: "harmonyos-sc", label: "HarmonyOS Sans", previewFamily: '"HarmonyOS Preview", "Microsoft YaHei UI", sans-serif', available: true },
      { id: "source-han-sans-cn", label: "思源黑体", previewFamily: '"Source Han Preview", "Microsoft YaHei UI", sans-serif', available: true },
      { id: "pingfang-sc", label: "苹方", previewFamily: '"PingFang SC", "Microsoft YaHei UI", sans-serif', available: false },
    ],
    activePresetId: null, activeFontLabel: "Windows 默认",
    currentPreviewFamily: '"Segoe UI", "Microsoft YaHei UI", sans-serif',
    backupDir: "", canRestore: false, pendingRecovery: null,
  };
}
export const applyFont = (presetId: string, paths: string[] = []) => invoke<ActionResult>("apply_font", { presetId, paths });
export const restoreFonts = (mode: "last" | "default") => invoke<ActionResult>("restore_fonts", { mode });
export const importFonts = (paths: string[]) => invoke<ActionResult>("import_fonts", { paths });
