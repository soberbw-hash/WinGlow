export type PageId = "home" | "fonts" | "menu" | "details" | "settings";
export interface FontPreset { id: string; label: string; previewFamily: string; available: boolean; }
export interface BootstrapPayload {
  presets: FontPreset[];
  activePresetId: string | null;
  activeFontLabel: string;
  currentPreviewFamily: string;
  currentPreviewWeight: number;
  needsFontRefresh: boolean;
  backupDir: string;
  canRestore: boolean;
  pendingRecovery: string | null;
}
export interface ActionResult { message: string; }
export interface ToggleState { id: string; label: string; enabled: boolean; note: string | null; }
export interface MenuItem {
  id: string; label: string; group: string; enabled: boolean; kind: string;
  rawLabel: string; autoHide: boolean; targets: string[]; menuLevel: "direct" | "cascade" | "extension";
  children: string[]; subItems: MenuItem[]; visibilityNote: string | null; iconDataUrl: string | null; iconSource: string | null;
}
export interface OptionalToolState { id: string; label: string; installed: boolean; managed: boolean; supported: boolean; note: string; }
export interface ShellState { items: MenuItem[]; tweaks: ToggleState[]; breezeEnabled: boolean; taskbarEnabled: boolean; taskbarSupported: boolean; startMenuEnabled: boolean; startMenuSupported: boolean; windowMaterialEnabled: boolean; windowMaterialSupported: boolean; optionalTools: OptionalToolState[]; optimizationActive: boolean; optimizationPending: boolean; pendingRecovery: boolean; }
export interface RepairProgress { running: boolean; stage: string; logDir: string; }
