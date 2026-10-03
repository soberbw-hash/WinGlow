export type PageId = "fonts" | "menu" | "details" | "settings";
export interface FontPreset { id: string; label: string; previewFamily: string; available: boolean; }
export interface BootstrapPayload {
  presets: FontPreset[];
  activePresetId: string | null;
  activeFontLabel: string;
  currentPreviewFamily: string;
  backupDir: string;
  canRestore: boolean;
  pendingRecovery: string | null;
}
export interface ActionResult { message: string; }
