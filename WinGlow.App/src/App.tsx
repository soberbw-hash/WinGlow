import { useEffect, useState } from "react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
import { Sparkles, Type, MousePointer2, PanelsTopLeft, RotateCcw, Minus, Square, X, Check } from "lucide-react";
import { MenuManager } from "./components/MenuManager";
import { AppUpdates } from "./components/AppUpdates";
import { VisualSettings } from "./components/VisualSettings";
import appIcon from "./assets/app-icon.png";
import { cn } from "./lib/cn";
import { openBackup, loadBootstrap, loadShell, applyFont, restoreFonts, setTweak, setAppearance, setMenuItem, restoreCategory, recoverPending, repairSystem, repairProgress, optimizeSystem, optimizeMenu, optionalTool, refreshMenuDictionary } from "./lib/tauri";
import type { BootstrapPayload, PageId, ShellState, ActionResult } from "./types";

const pages = [
  { id: "home", label: "一键优化", icon: Sparkles },
  { id: "fonts", label: "字体", icon: Type },
  { id: "menu", label: "右键菜单", icon: MousePointer2 },
  { id: "details", label: "基础美化", icon: PanelsTopLeft },
] as const;
function FontSample({ family, weight = 400 }: { family: string; weight?: number }) {
  return <div className="font-sample" style={{ fontFamily: family, fontWeight: weight }}>
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
  const [page, setPage] = useState<PageId>("home");
  const [bootstrap, setBootstrap] = useState<BootstrapPayload | null>(null);
  const [shell, setShell] = useState<ShellState | null>(null);
  const [selectedId, setSelectedId] = useState("harmonyos-sc-bold");
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [repairing, setRepairing] = useState(false);
  const [repairStage, setRepairStage] = useState("");
  const [notice, setNotice] = useState<{ text: string; error: boolean } | null>(null);
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
  const harmonySelected = selectedId === "harmonyos-sc" || selectedId === "harmonyos-sc-bold";
  const familyPresets = bootstrap?.presets.filter(p => p.id !== "harmonyos-sc-bold") ?? [];
  const pendingRecovery = !!bootstrap?.pendingRecovery || !!shell?.pendingRecovery || !!shell?.optimizationPending;
  const blocked = !desktop || busy || loading || pendingRecovery;
  async function mutate(operation: () => Promise<ActionResult>) {
    if (busy) return;
    setBusy(true); setNotice(null);
    try { const result = await operation(); if (result.message) setNotice({ text: result.message, error: false }); }
    catch (error) { setNotice({ text: String(error), error: true }); }
    finally { await reload(); setBusy(false); }
  }
  async function applySelected() {
    if (!selected || busy) return;
    await mutate(() => applyFont(selected.id));
  }
  async function scanRepair() {
    setRepairing(true); setRepairStage("正在准备扫描…");
    try { await mutate(repairSystem); } finally { setRepairing(false); }
  }
  async function windowAction(action: "minimize" | "toggleMaximize" | "close") {
    if (desktop && !(busy && action === "close")) await getCurrentWindow()[action]();
  }
  function navigate(next: PageId) { setPage(next); setNotice(null); }
  return <div className="app-shell">
    <header className="titlebar" data-tauri-drag-region><span className="titlebar-name" data-tauri-drag-region>WinGlow</span><div className="window-controls">
      <button aria-label="最小化" onClick={() => void windowAction("minimize")}><Minus size={15} /></button><button aria-label="最大化或还原" onClick={() => void windowAction("toggleMaximize")}><Square size={12} /></button><button aria-label="关闭" className="close-window" disabled={busy} onClick={() => void windowAction("close")}><X size={17} /></button>
    </div></header>
    <div className="workspace"><aside className="sidebar"><div className="brand"><img src={appIcon} alt="" width={40} height={40} /><span>WinGlow</span></div>
      <nav aria-label="功能">{pages.map(({ id, label, icon: Icon }) => <button key={id} className={cn("nav-item", page === id && "active")} aria-current={page === id ? "page" : undefined} onClick={() => navigate(id)}><Icon size={18} /><span>{label}</span></button>)}</nav>
      <div className="sidebar-bottom"><button className={cn("nav-item", page === "settings" && "active")} aria-current={page === "settings" ? "page" : undefined} onClick={() => navigate("settings")}><RotateCcw size={18} /><span>设置与还原</span></button></div>
    </aside><main className={cn("main-content", page === "menu" && "menu-page", page === "home" && "home-main")} aria-busy={loading || busy}>
      {page === "home" ? <section className="home-page" aria-label="一键优化">
        <div className="home-intro"><img src={appIcon} alt="" width={96} height={96} /><h1>让电脑更美观</h1><p>舒服的字体，清爽的菜单，柔和的磨砂效果。</p></div>
        {!shell && !loading && <button className="button secondary" onClick={() => void reload()}>重新读取</button>}
        <div className="home-actions"><button className="button primary optimize-button" disabled={blocked || !shell || shell.optimizationActive} title="首次使用需联网准备 Breeze。完成后自动刷新资源管理器，文件窗口可能关闭。" onClick={() => void mutate(() => optimizeSystem())}><Sparkles size={20} />{busy ? "正在处理…" : shell?.optimizationActive && !shell.optimizationPending ? "已优化" : "一键优化"}</button>
          {shell?.optimizationActive && !shell.optimizationPending && <button className="button secondary home-restore" disabled={!desktop || busy || loading} onClick={() => void mutate(() => optimizeSystem(true))}><RotateCcw size={17} />撤销优化</button>}
        </div>
        {shell && !shell.taskbarSupported && <p className="home-support">透明任务栏需要 Windows 11。</p>}
      </section> : page === "fonts" ? <><h1>字体</h1>
        {loading && !bootstrap ? <div className="empty-state"><p>正在读取…</p></div> : !bootstrap ? <div className="empty-state"><button className="button secondary" onClick={() => void reload(true)}>重试</button></div> : <>
          <fieldset className="font-picker" disabled={busy || loading}><legend className="sr-only">选择字体</legend><div className="font-options">{familyPresets.map(preset => {
            const checked = preset.id === "harmonyos-sc" ? harmonySelected : selectedId === preset.id;
            return <label key={preset.id} className={cn("font-option", checked && "selected")}><input type="radio" name="font" value={preset.id} checked={checked} onChange={() => { setSelectedId(preset.id === "harmonyos-sc" ? "harmonyos-sc-bold" : preset.id); setNotice(null); }} /><span>{preset.label}</span><Check size={16} aria-hidden="true" className="option-check" /></label>;
          })}</div></fieldset>
          {harmonySelected && <fieldset className="font-weight-picker" disabled={busy || loading}><legend className="sr-only">鸿蒙字重</legend>{[{id:"harmonyos-sc",label:"标准"},{id:"harmonyos-sc-bold",label:"粗"}].map(style => <label className={cn("font-weight-option", selectedId === style.id && "selected")} key={style.id}><input type="radio" name="harmony-weight" checked={selectedId === style.id} onChange={() => { setSelectedId(style.id); setNotice(null); }} /><span>{style.label}</span></label>)}</fieldset>}
          <div className="comparison"><section className="preview-panel" aria-label="更换前"><div className="preview-caption"><h2>更换前</h2></div><FontSample family={bootstrap.currentPreviewFamily} weight={bootstrap.currentPreviewWeight} /></section><section className="preview-panel" aria-label="更换后"><div className="preview-caption"><h2>更换后</h2></div><FontSample family={selected?.previewFamily ?? '"Microsoft YaHei UI", sans-serif'} /></section></div>
          <div className="apply-row"><button className="button primary apply-button" title="应用成功后自动重启资源管理器，桌面和任务栏会短暂消失，文件窗口可能关闭。请先完成文件复制。" disabled={blocked || !selected || bootstrap.activePresetId === selectedId && !bootstrap.needsFontRefresh} onClick={() => void applySelected()}>{busy ? "处理中…" : bootstrap.activePresetId === selectedId && !bootstrap.needsFontRefresh ? <><Check size={16} />已应用</> : "应用"}</button></div>
        </>}</> : page === "menu" ? <>
          <div className="menu-page-heading"><h1>右键菜单</h1><button className="button primary menu-auto-button" disabled={blocked || !shell} onClick={() => void mutate(optimizeMenu)}>一键精简</button><div className="breeze-compact" title="首次开启需下载，关闭后注销恢复原菜单。"><span>Breeze 美化</span><Switch label="Breeze 美化" checked={shell?.breezeEnabled ?? false} disabled={blocked || !shell} onChange={value => void mutate(() => setTweak("breeze", value))} /></div></div>
          {!shell && loading ? <p className="empty-state">正在读取…</p> : !shell ? <button className="button secondary" onClick={() => void reload()}>重试</button> : <MenuManager items={shell.items} disabled={blocked} loading={busy || loading} onRefresh={() => void reload()} onToggle={(id, enabled) => void mutate(() => setMenuItem(id, enabled))} />}
        </> : page === "details" ? <><h1>基础美化</h1>{!shell ? <div className="empty-state"><button className="button secondary" disabled={loading} onClick={() => void reload()}>重试</button></div> : <>
          <div className="settings-list">
            {[{ id: "transparent-taskbar", label: "透明任务栏", note: "柔和模糊背景，保留任务栏细线。", enabled: shell.taskbarEnabled, supported: shell.taskbarSupported }, { id: "window-material", label: "窗口背景", note: "调整支持窗口的背景材质。不透明应用可能看不到变化。", enabled: shell.windowMaterialEnabled, supported: shell.windowMaterialSupported }, { id: "start-menu", label: "开始菜单美化", note: "调整开始菜单的磨砂背景、圆角与鸿蒙字体，保留原来的布局。背景浓度越低，越通透。", enabled: shell.startMenuEnabled, supported: shell.startMenuSupported }].map(effect => <section className="effect-section" key={effect.id}><div className="setting-row"><div><h2>{effect.label}</h2><p>{effect.supported ? effect.note : "当前 Windows 版本暂不支持。"}</p></div><Switch label={effect.label} checked={effect.enabled} disabled={blocked || !effect.supported} onChange={value => void mutate(() => setTweak(effect.id, value))} /></div>{effect.supported && (effect.id === "window-material" || effect.id === "start-menu") && <VisualSettings key={effect.id === "window-material" ? shell.appearance.material : `${shell.appearance.tint}-${shell.appearance.radius}`} id={effect.id} settings={shell.appearance} enabled={effect.enabled} disabled={blocked} onApply={settings => void mutate(() => setAppearance(effect.id, settings))} />}</section>)}
            {shell.tweaks.map(tweak => <div className="setting-row" key={tweak.id}><div><h2>{tweak.label}</h2>{tweak.note && <p>{tweak.note}</p>}</div><Switch label={tweak.label} checked={tweak.enabled} disabled={blocked} onChange={value => void mutate(() => setTweak(tweak.id, value))} /></div>)}
          </div>
          <section className="optional-section" aria-labelledby="optional-heading"><h2 id="optional-heading">可选功能</h2>
            {shell.optionalTools.map(tool => <div className="setting-row" key={tool.id}><div><h3>{tool.label}</h3><p>{tool.note}</p></div><div className="optional-actions"><button className="button secondary" disabled={blocked || !tool.supported} onClick={() => void mutate(() => optionalTool(tool.id, tool.installed ? "open" : "install"))}>{tool.installed ? "打开" : "安装"}</button>{tool.managed && <RestoreButton label="还原" description={`卸载 WinGlow 安装的${tool.label}，恢复启用前的配置。`} disabled={blocked} onRestore={() => void mutate(() => optionalTool(tool.id, "remove"))} />}</div></div>)}
          </section>
        </>}</> : <>
          <h1>还原</h1><div className="restore-actions">
            <RestoreButton label="恢复默认字体" description="恢复默认字体映射和原来的显示参数，修改前自动备份。成功后重启资源管理器，请先完成文件复制。" disabled={!desktop || busy || loading} onRestore={() => void mutate(() => restoreFonts("default"))} />
            <RestoreButton label="还原右键菜单" description="还原本工具修改的菜单项与 Breeze 自启设置。关闭 Breeze 后需要注销。" disabled={!desktop || busy || loading} onRestore={() => void mutate(() => restoreCategory("menu"))} />
            <RestoreButton label="还原基础美化" description="恢复本工具修改前的基础美化设置。" disabled={!desktop || busy || loading} onRestore={() => void mutate(() => restoreCategory("details"))} />
            <RestoreButton label="撤销最近修改" description="回到最近一次操作前。成功后重启资源管理器，请先完成文件复制。" disabled={!desktop || busy || loading} onRestore={() => void mutate(() => restoreCategory("all-last"))} />
          </div><div className="repair-section"><h2>普通还原无效？</h2><p>扫描并修复 Windows 系统文件，再恢复默认字体。可能需要联网和重启。</p><RestoreButton label="扫描并修复" description="将运行 Windows 系统修复，可能需要较长时间。期间不要关闭程序，结束后请注销或重启。" disabled={!desktop || busy || loading} primary onRestore={() => void scanRepair()} /></div>
          <button className="text-button" disabled={!desktop || !bootstrap} onClick={() => { if (bootstrap) void openBackup().catch(error => setNotice({ text: String(error), error: true })); }}>打开备份</button>
          <div className="dictionary-settings"><span>菜单识别库</span><button className="button secondary" disabled={blocked} onClick={() => void mutate(refreshMenuDictionary)}>检查更新</button></div>
        </>}
      {pendingRecovery && <div className="notice error" role="alert"><p>{bootstrap?.pendingRecovery || "上次修改没有完成，请先还原。"}</p><button className="button secondary" disabled={busy || loading || !desktop} onClick={() => void mutate(recoverPending)}>{busy ? "正在恢复…" : "恢复未完成修改"}</button></div>}
      <AppUpdates settings={page === "settings"} blocked={busy || repairing || !!bootstrap?.pendingRecovery || !!shell?.pendingRecovery || !!shell?.optimizationPending} />
      {repairing && <div className="notice" role="status">{repairStage}</div>}
      {notice && <div className={cn("notice", notice.error && "error")} role={notice.error ? "alert" : "status"}>{notice.text}</div>}
    </main></div>
  </div>;
}
