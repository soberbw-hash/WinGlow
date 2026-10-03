import { useEffect, useState } from "react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import { open as selectFiles } from "@tauri-apps/plugin-dialog";
import { Type, MousePointer2, PanelsTopLeft, RotateCcw, Minus, Square, X, Check, RefreshCw } from "lucide-react";
import appIcon from "./assets/app-icon.png";
import { cn } from "./lib/cn";
import { loadBootstrap, loadShell, applyFont, restoreFonts, importFonts, setTweak, setMenuItem, restoreCategory, repairSystem, repairProgress } from "./lib/tauri";
import type { BootstrapPayload, PageId, ShellState, ActionResult } from "./types";

const pages = [
  { id: "fonts", label: "字体", icon: Type },
  { id: "menu", label: "右键菜单", icon: MousePointer2 },
  { id: "details", label: "基础美化", icon: PanelsTopLeft },
] as const;
function FontSample({ family }: { family: string }) {
  return <div className="font-sample" style={{ fontFamily: family }}>
    <div className="sample-heading">每一天，从桌面开始。</div>
    <p className="sample-paragraph">打开电脑，继续未完成的事。</p>
    <p className="sample-english">Windows · Aa Bb Gg · 0123456789</p>
  </div>;
}
function Switch({ label, checked, disabled, onChange }: { label: string; checked: boolean; disabled: boolean; onChange: (checked: boolean) => void }) {
  return <label className="switch"><input type="checkbox" role="switch" aria-label={label} checked={checked} disabled={disabled} onChange={e => onChange(e.target.checked)} /><span aria-hidden="true" /></label>;
}
function RestoreButton({ label, description, disabled, onRestore, primary = false }: {
  label: string; description: string; disabled: boolean; onRestore: () => void; primary?: boolean;
}) {
  return <AlertDialog.Root>
    <AlertDialog.Trigger asChild><button className={cn("button", primary ? "primary" : "secondary")} disabled={disabled}>{label}</button></AlertDialog.Trigger>
    <AlertDialog.Portal><AlertDialog.Overlay className="dialog-overlay" /><AlertDialog.Content className="dialog-content">
      <AlertDialog.Title className="dialog-title">{label}？</AlertDialog.Title>
      <AlertDialog.Description className="dialog-description">{description}</AlertDialog.Description>
      <div className="dialog-actions"><AlertDialog.Cancel asChild><button className="button secondary">取消</button></AlertDialog.Cancel><AlertDialog.Action asChild><button className="button primary" onClick={onRestore}>继续</button></AlertDialog.Action></div>
    </AlertDialog.Content></AlertDialog.Portal>
  </AlertDialog.Root>;
}
export default function App() {
  const [page, setPage] = useState<PageId>("fonts");
  const [bootstrap, setBootstrap] = useState<BootstrapPayload | null>(null);
  const [shell, setShell] = useState<ShellState | null>(null);
  const [selectedId, setSelectedId] = useState("harmonyos-sc");
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [repairing, setRepairing] = useState(false);
  const [repairStage, setRepairStage] = useState("");
  const [notice, setNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [search, setSearch] = useState("");
  const desktop = isTauri();
  async function reload(initial = false) {
    setLoading(true);
    const results = await Promise.allSettled([loadBootstrap(), loadShell()]);
    if (results[0].status === "fulfilled") {
      setBootstrap(results[0].value);
      if (initial && results[0].value.activePresetId) setSelectedId(results[0].value.activePresetId);
    } else setNotice({ text: String(results[0].reason), error: true });
    if (results[1].status === "fulfilled") setShell(results[1].value);
    else setNotice({ text: String(results[1].reason), error: true });
    setLoading(false);
  }
  useEffect(() => { void reload(true); }, []);
  useEffect(() => {
    if (!desktop || !busy) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void getCurrentWindow().onCloseRequested(event => event.preventDefault()).then(off => {
      if (disposed) off(); else unlisten = off;
    });
    return () => { disposed = true; unlisten?.(); };
  }, [desktop, busy]);
  useEffect(() => {
    if (!repairing || !desktop) return;
    let live = true;
    const timer = window.setInterval(() => { void repairProgress().then(p => { if (live && p.stage) setRepairStage(p.stage); }).catch(() => {}); }, 1000);
    return () => { live = false; window.clearInterval(timer); };
  }, [repairing, desktop]);
  const selected = bootstrap?.presets.find(p => p.id === selectedId);
  const blocked = !desktop || busy || loading || !!bootstrap?.pendingRecovery || !!shell?.pendingRecovery;
  async function mutate(operation: () => Promise<ActionResult>) {
    if (busy) return;
    setBusy(true); setNotice(null);
    try { const result = await operation(); if (result.message) setNotice({ text: result.message, error: false }); }
    catch (error) { setNotice({ text: String(error), error: true }); }
    finally { await reload(); setBusy(false); }
  }
  async function applySelected() {
    if (!selected || busy) return;
    await mutate(async () => {
      if (selected.id === "pingfang-sc" && !selected.available) {
        const files = await selectFiles({ title: "选择苹方字体（常规与粗体）", multiple: true, filters: [{ name: "字体", extensions: ["ttf", "otf", "ttc", "otc"] }] });
        if (!files) return { message: "" };
        return importFonts(typeof files === "string" ? [files] : files);
      }
      return applyFont(selected.id);
    });
  }
  async function scanRepair() {
    setRepairing(true); setRepairStage("正在准备扫描…");
    try { await mutate(repairSystem); } finally { setRepairing(false); }
  }
  async function windowAction(action: "minimize" | "toggleMaximize" | "close") {
    if (desktop && !(busy && action === "close")) await getCurrentWindow()[action]();
  }
  function navigate(next: PageId) { setPage(next); setNotice(null); }
  const items = shell?.items.filter(item => (item.label + item.group).toLocaleLowerCase().includes(search.toLocaleLowerCase())) ?? [];
  return <div className="app-shell">
    <header className="titlebar" data-tauri-drag-region><span className="titlebar-name" data-tauri-drag-region>WinGlow</span><div className="window-controls">
      <button aria-label="最小化" onClick={() => void windowAction("minimize")}><Minus size={15} /></button><button aria-label="最大化或还原" onClick={() => void windowAction("toggleMaximize")}><Square size={12} /></button><button aria-label="关闭" className="close-window" disabled={busy} onClick={() => void windowAction("close")}><X size={17} /></button>
    </div></header>
    <div className="workspace"><aside className="sidebar"><div className="brand"><img src={appIcon} alt="" width={40} height={40} /><span>WinGlow</span></div>
      <nav aria-label="功能">{pages.map(({ id, label, icon: Icon }) => <button key={id} className={cn("nav-item", page === id && "active")} aria-current={page === id ? "page" : undefined} onClick={() => navigate(id)}><Icon size={18} /><span>{label}</span></button>)}</nav>
      <div className="sidebar-bottom"><button className={cn("nav-item", page === "settings" && "active")} aria-current={page === "settings" ? "page" : undefined} onClick={() => navigate("settings")}><RotateCcw size={18} /><span>还原</span></button></div>
    </aside><main className="main-content" aria-busy={loading || busy}>
      {page === "fonts" ? <><h1>字体</h1>
        {loading && !bootstrap ? <div className="empty-state"><p>正在读取…</p></div> : !bootstrap ? <div className="empty-state"><button className="button secondary" onClick={() => void reload(true)}>重试</button></div> : <>
          <fieldset className="font-picker" disabled={busy || loading}><legend className="sr-only">选择字体</legend><div className="font-options">{bootstrap.presets.map(preset => <label key={preset.id} className={cn("font-option", selectedId === preset.id && "selected")}><input type="radio" name="font" value={preset.id} checked={selectedId === preset.id} onChange={() => { setSelectedId(preset.id); setNotice(null); }} /><span>{preset.label}</span><Check size={16} aria-hidden="true" className="option-check" /></label>)}</div></fieldset>
          <div className="comparison"><section className="preview-panel" aria-label="更换前"><div className="preview-caption"><h2>更换前</h2></div><FontSample family={bootstrap.currentPreviewFamily} /></section><section className="preview-panel" aria-label="更换后"><div className="preview-caption"><h2>更换后</h2></div>{selected?.id === "pingfang-sc" && !selected.available ? <div className="font-sample preview-unavailable"><p>先导入苹方字体</p></div> : <FontSample family={selected?.previewFamily ?? '"Microsoft YaHei UI", sans-serif'} />}</section></div>
          <div className="apply-row"><button className="button primary apply-button" title="应用成功后自动重启资源管理器，桌面和任务栏会短暂消失，文件窗口可能关闭。请先完成文件复制。" disabled={blocked || !selected || bootstrap.activePresetId === selectedId} onClick={() => void applySelected()}>{busy ? "处理中…" : bootstrap.activePresetId === selectedId ? <><Check size={16} />已应用</> : selected?.id === "pingfang-sc" && !selected.available ? "导入苹方" : "应用"}</button></div>
        </>}</> : page === "menu" ? <>
          <h1>右键菜单</h1><div className="setting-row breeze-row"><div><h2>Breeze 美化</h2><p>首次开启需下载，关闭后注销恢复原菜单。</p></div><Switch label="Breeze 美化" checked={shell?.breezeEnabled ?? false} disabled={blocked || !shell} onChange={value => void mutate(() => setTweak("breeze", value))} /></div>
          <div className="menu-toolbar"><input type="search" aria-label="查找菜单项" placeholder="查找菜单项" value={search} onChange={e => setSearch(e.target.value)} /><button className="icon-button" aria-label="刷新菜单项" disabled={busy || loading} onClick={() => void reload()}><RefreshCw size={17} /></button></div>
          {!shell && loading ? <p className="empty-state">正在读取…</p> : !shell ? <button className="button secondary" onClick={() => void reload()}>重试</button> : items.length === 0 ? <p className="empty-state">{search ? "没有匹配的菜单项" : "没有可管理的菜单项"}</p> : <div className="settings-list">{items.map(item => <div className="setting-row" key={item.id}><div><h2>{item.label}</h2><p>{item.group} · {item.kind}</p></div><Switch label={item.label} checked={item.enabled} disabled={blocked} onChange={value => void mutate(() => setMenuItem(item.id, value))} /></div>)}</div>}
        </> : page === "details" ? <><h1>基础美化</h1>{!shell ? <div className="empty-state"><button className="button secondary" disabled={loading} onClick={() => void reload()}>重试</button></div> : <div className="settings-list">{shell.tweaks.map(tweak => <div className="setting-row" key={tweak.id}><div><h2>{tweak.label}</h2>{tweak.note && <p>{tweak.note}</p>}</div><Switch label={tweak.label} checked={tweak.enabled} disabled={blocked} onChange={value => void mutate(() => setTweak(tweak.id, value))} /></div>)}</div>}</> : <>
          <h1>还原</h1><div className="restore-actions">
            <RestoreButton label="恢复默认字体" description="恢复默认字体映射和原来的显示参数，修改前自动备份。成功后重启资源管理器，请先完成文件复制。" disabled={!desktop || busy || loading} onRestore={() => void mutate(() => restoreFonts("default"))} />
            <RestoreButton label="还原右键菜单" description="还原本工具修改的菜单项与 Breeze 自启设置。关闭 Breeze 后需要注销。" disabled={!desktop || busy || loading} onRestore={() => void mutate(() => restoreCategory("menu"))} />
            <RestoreButton label="还原基础美化" description="恢复本工具修改前的基础美化设置。" disabled={!desktop || busy || loading} onRestore={() => void mutate(() => restoreCategory("details"))} />
            <RestoreButton label="撤销最近修改" description="回到最近一次操作前。成功后重启资源管理器，请先完成文件复制。" disabled={!desktop || busy || loading} onRestore={() => void mutate(() => restoreCategory("all-last"))} />
          </div><div className="repair-section"><h2>普通还原无效？</h2><p>扫描并修复 Windows 系统文件，再恢复默认字体。可能需要联网和重启。</p><RestoreButton label="扫描并修复" description="将运行 Windows 系统修复，可能需要较长时间。期间不要关闭程序，结束后请注销或重启。" disabled={!desktop || busy || loading} primary onRestore={() => void scanRepair()} /></div>
          <button className="text-button" disabled={!desktop || !bootstrap} onClick={() => { if (bootstrap) void openPath(bootstrap.backupDir).catch(error => setNotice({ text: String(error), error: true })); }}>打开备份</button>
        </>}
      {(bootstrap?.pendingRecovery || shell?.pendingRecovery) && <div className="notice error" role="alert"><p>{bootstrap?.pendingRecovery || "上次修改没有完成，请先还原。"}</p><button className="button secondary" disabled={busy || !desktop} onClick={() => { navigate("settings"); }}>去还原</button></div>}
      {repairing && <div className="notice" role="status">{repairStage}</div>}
      {notice && <div className={cn("notice", notice.error && "error")} role={notice.error ? "alert" : "status"}>{notice.text}</div>}
    </main></div>
  </div>;
}
