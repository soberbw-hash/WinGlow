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
export interface ToggleState { id: string; label: string; enabled: boolean; note: string | null; }
export interface MenuItem { id: string; label: string; group: string; enabled: boolean; kind: string; }
export interface ShellState { items: MenuItem[]; tweaks: ToggleState[]; breezeEnabled: boolean; pendingRecovery: boolean; }
export interface RepairProgress { running: boolean; stage: string; logDir: string; }
