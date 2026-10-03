import { useEffect, useState } from "react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl, openPath } from "@tauri-apps/plugin-opener";
import { open as selectFiles } from "@tauri-apps/plugin-dialog";
import { Type, MousePointer2, PanelsTopLeft, RotateCcw, Minus, Square, X, ArrowUpRight, Check } from "lucide-react";
import appIcon from "./assets/app-icon.png";
import { cn } from "./lib/cn";
import { loadBootstrap, applyFont, restoreFonts, importFonts } from "./lib/tauri";
import type { BootstrapPayload, PageId } from "./types";

const RELEASE_URL = "https://github.com/soberbw-hash/WindowsFontTuner/releases";
const pages = [
  { id: "fonts", label: "字体", icon: Type },
  { id: "menu", label: "右键菜单", icon: MousePointer2 },
  { id: "details", label: "界面细节", icon: PanelsTopLeft },
] as const;

function FontSample({ family }: { family: string }) {
  return <div className="font-sample" style={{ fontFamily: family }}>
    <div className="sample-heading">桌面、文件与设置</div>
    <p className="sample-paragraph">这是一段中文文字，用来比较字体的笔画与间距。<br />看看常用的小字，是否也合你的心意。</p>
    <p className="sample-english">Windows 11 · Settings &amp; Files</p>
    <p className="sample-numbers">0123456789&nbsp;&nbsp; Aa Bb Gg&nbsp;&nbsp; 1Il 0O</p>
    <div className="sample-divider" />
    <div className="sample-row"><span>文档</span><span>2026/10/03</span></div>
    <div className="sample-row"><span>下载</span><span>128 KB</span></div>
    <p className="sample-small">常用的小字号文字 · Regular / <strong>Bold</strong></p>
  </div>;
}

function RestoreButton({ label, description, disabled, onRestore }: {
  label: string; description: string; disabled: boolean; onRestore: () => void;
}) {
  return <AlertDialog.Root>
    <AlertDialog.Trigger asChild><button className="button secondary" disabled={disabled}>{label}</button></AlertDialog.Trigger>
    <AlertDialog.Portal>
      <AlertDialog.Overlay className="dialog-overlay" />
      <AlertDialog.Content className="dialog-content">
        <AlertDialog.Title className="dialog-title">{label}？</AlertDialog.Title>
        <AlertDialog.Description className="dialog-description">{description}本次修改前会自动备份。</AlertDialog.Description>
        <div className="dialog-actions">
          <AlertDialog.Cancel asChild><button className="button secondary">取消</button></AlertDialog.Cancel>
          <AlertDialog.Action asChild><button className="button primary" onClick={onRestore}>确认恢复</button></AlertDialog.Action>
        </div>
      </AlertDialog.Content>
    </AlertDialog.Portal>
  </AlertDialog.Root>;
}

export default function App() {
  const [page, setPage] = useState<PageId>("fonts");
  const [bootstrap, setBootstrap] = useState<BootstrapPayload | null>(null);
  const [selectedId, setSelectedId] = useState("harmonyos-sc");
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [notice, setNotice] = useState<{ text: string; error: boolean } | null>(null);
  const desktop = isTauri();
  async function reload(initial = false) {
    setLoading(true);
    try {
      const data = await loadBootstrap(); setBootstrap(data);
      if (initial && data.activePresetId) setSelectedId(data.activePresetId);
    } catch (error) { setNotice({ text: `读取系统字体失败：${String(error)}`, error: true }); }
    finally { setLoading(false); }
  }
  useEffect(() => { void reload(true); }, []);
  const selected = bootstrap?.presets.find(preset => preset.id === selectedId);
  async function mutate(operation: () => Promise<{ message: string }>) {
    if (busy) return;
    setBusy(true); setNotice(null);
    try { const result = await operation(); setNotice({ text: result.message, error: false }); }
    catch (error) { setNotice({ text: String(error), error: true }); }
    finally { await reload(); setBusy(false); }
  }
  async function applySelected() {
    if (!selected || busy) return;
    // Keep the entire file-dialog + elevation operation locked against double clicks.
    await mutate(async () => {
      let paths: string[] = [];
      if (selected.id === "pingfang-sc" && !selected.available) {
        const files = await selectFiles({ title: "选择你有权使用的苹方字体（含常规与粗体）", multiple: true, filters: [{ name: "字体", extensions: ["ttf", "otf", "ttc", "otc"] }] });
        if (!files) return { message: "已取消导入。" };
        paths = typeof files === "string" ? [files] : files;
        return importFonts(paths);
      }
      return applyFont(selected.id, paths);
    });
  }
  async function windowAction(action: "minimize" | "toggleMaximize" | "close") {
    if (desktop && !(busy && action === "close")) await getCurrentWindow()[action]();
  }
  async function showReleases() {
    if (desktop) await openUrl(RELEASE_URL);
    else window.open(RELEASE_URL, "_blank", "noopener,noreferrer");
  }
  return <div className="app-shell">
    <header className="titlebar" data-tauri-drag-region>
      <span className="titlebar-name" data-tauri-drag-region>Windows 微调</span>
      <div className="window-controls">
        <button aria-label="最小化" onClick={() => void windowAction("minimize")}><Minus size={15} /></button>
        <button aria-label="最大化或还原" onClick={() => void windowAction("toggleMaximize")}><Square size={12} /></button>
        <button aria-label="关闭" className="close-window" disabled={busy} onClick={() => void windowAction("close")}><X size={17} /></button>
      </div>
    </header>
    <div className="workspace">
      <aside className="sidebar">
        <div className="brand"><img src={appIcon} alt="" width={34} height={34} /><span>Windows 微调</span></div>
        <nav aria-label="功能">{pages.map(({ id, label, icon: Icon }) => <button key={id} className={cn("nav-item", page === id && "active")} aria-current={page === id ? "page" : undefined} onClick={() => setPage(id)}><Icon size={18} /><span>{label}</span></button>)}</nav>
        <div className="sidebar-bottom">
          <button className={cn("nav-item", page === "settings" && "active")} aria-current={page === "settings" ? "page" : undefined} onClick={() => setPage("settings")}><RotateCcw size={18} /><span>恢复与设置</span></button>
          <button className="release-link" onClick={() => void showReleases()}>发布页<ArrowUpRight size={14} /></button>
        </div>
      </aside>
      <main className="main-content" aria-busy={loading || busy}>
        {page === "fonts" ? <>
          <h1>字体</h1>
          {loading && !bootstrap ? <div className="empty-state"><p>正在读取系统字体…</p></div> : !bootstrap ? <div className="empty-state"><p>暂时无法读取字体。</p><button className="button secondary" onClick={() => void reload(true)}>重试</button></div> : <>
            <fieldset className="font-picker" disabled={busy || loading}><legend>更换为</legend>
              <div className="font-options">{bootstrap.presets.map(preset => <label key={preset.id} className={cn("font-option", selectedId === preset.id && "selected")}>
                <input type="radio" name="font" value={preset.id} checked={selectedId === preset.id} onChange={() => { setSelectedId(preset.id); setNotice(null); }} />
                <span>{preset.label}</span><Check size={16} aria-hidden="true" className="option-check" />
              </label>)}</div>
            </fieldset>
            <div className="comparison">
              <section className="preview-panel" aria-label="更换前"><div className="preview-caption"><h2>更换前</h2><span>{bootstrap.activeFontLabel}</span></div><FontSample family={bootstrap.currentPreviewFamily} /></section>
              <section className="preview-panel" aria-label="更换后"><div className="preview-caption"><h2>更换后</h2><span>{selected?.label}</span></div>{selected?.id === "pingfang-sc" && !selected.available ? <div className="font-sample preview-unavailable"><p>导入苹方后显示预览</p><span>需要你有权使用的常规与粗体字体文件。</span></div> : <FontSample family={selected?.previewFamily ?? '"Microsoft YaHei UI", sans-serif'} />}</section>
            </div>
            <div className="apply-row"><p>{desktop ? "预览仅比较字形，系统效果因应用而异。" : "界面预览 · 请在 Windows 应用中应用字体。"}</p><button className="button primary apply-button" disabled={!desktop || !selected || loading || busy || bootstrap.activePresetId === selectedId || !!bootstrap.pendingRecovery} onClick={() => void applySelected()}>{busy ? "正在处理…" : bootstrap.activePresetId === selectedId ? <><Check size={16} />已应用</> : selected?.id === "pingfang-sc" && !selected.available ? "导入苹方" : "应用"}</button></div>
          </>}
        </> : page === "settings" ? <>
          <h1>恢复与设置</h1>
          <div className="settings-list">
            <div className="setting-row"><div><h2>恢复上次修改</h2><p>回到最近一次修改前的字体设置。</p></div><RestoreButton label="恢复上次修改" description="将恢复最近一次修改前的字体设置。" disabled={!desktop || busy || loading || !bootstrap?.canRestore} onRestore={() => void mutate(() => restoreFonts("last"))} /></div>
            <div className="setting-row"><div><h2>恢复 Windows 默认字体</h2><p>恢复本工具管理的字体映射，保留系统原始字体。</p></div><RestoreButton label="恢复默认" description="将清除本工具管理的字体替换。" disabled={!desktop || busy || loading || !bootstrap} onRestore={() => void mutate(() => restoreFonts("default"))} /></div>
            <div className="setting-row"><div><h2>自动备份</h2><p>每次修改前保存，写入失败自动回退。</p></div><button className="button secondary" disabled={!desktop || !bootstrap} onClick={() => { if (bootstrap) void openPath(bootstrap.backupDir).catch(error => setNotice({ text: String(error), error: true })); }}>打开备份</button></div>
          </div>
          <p className="settings-hint">需要急救时，按住 Shift 启动程序，恢复上次修改前的设置。</p>
          <div className="about"><img src={appIcon} width={32} height={32} alt="" /><div><strong>Windows 微调</strong><p>让 Windows 更合心意。调整字体、右键菜单与界面细节。</p><span>3.0.0 · Rust + Tauri 2</span></div></div>
        </> : <><h1>{page === "menu" ? "右键菜单" : "界面细节"}</h1><div className="empty-state"><p>这项功能将在后续版本提供。</p><button className="button secondary" onClick={() => setPage("fonts")}>先调整字体</button></div></>}
        {bootstrap?.pendingRecovery && <div className="notice error" role="alert"><p>{bootstrap.pendingRecovery}</p><button className="button secondary" disabled={busy || !desktop} onClick={() => void mutate(() => restoreFonts("last"))}>恢复未完成的修改</button></div>}
        {notice && <div className={cn("notice", notice.error && "error")} role={notice.error ? "alert" : "status"}>{notice.text}</div>}
      </main>
    </div>
  </div>;
}
