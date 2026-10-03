import { useEffect, useState } from "react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { invoke, isTauri } from "@tauri-apps/api/core";

type UpdateState = { currentVersion: string; phase: string; version: string | null; downloaded: number; total: number | null; error: string | null };
const initial: UpdateState = { currentVersion: "—", phase: "idle", version: null, downloaded: 0, total: null, error: null };
export function AppUpdates({ settings, blocked }: { settings: boolean; blocked: boolean }) {
  const [state, setState] = useState(initial);
  const [open, setOpen] = useState(false);
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const desktop = isTauri();
  useEffect(() => {
    if (!desktop) return;
    let live = true;
    const poll = () => { void invoke<UpdateState>("update_status").then(value => { if (live) setState(value); }).catch(() => {}); };
    poll();
    const timer = window.setInterval(poll, 1500);
    return () => { live = false; window.clearInterval(timer); };
  }, [desktop]);
  useEffect(() => {
    if (state.phase === "ready" && state.version !== dismissed && !blocked) setOpen(true);
  }, [state.phase, state.version, dismissed, blocked]);
  async function check() {
    setError(null);
    try {
      const next = await invoke<UpdateState>("check_updates");
      setState(next);
      if (next.phase === "ready") setOpen(true);
    } catch { setError("检查更新失败，请稍后重试。"); }
  }
  async function install() {
    setInstalling(true); setError(null);
    try { await invoke("install_update"); }
    catch (failure) { setError(String(failure)); setInstalling(false); }
  }
  const active = state.phase === "checking" || state.phase === "downloading" || state.phase === "installing";
  const status = state.phase === "checking" ? "正在检查更新…" : state.phase === "downloading" ? `正在后台下载${state.total ? ` ${Math.min(100, Math.floor(state.downloaded / state.total * 100))}%` : "…"}` : state.phase === "ready" ? `新版本 ${state.version} 已下载` : state.phase === "current" ? "已是最新版本" : state.phase === "error" ? "检查或下载失败，请重试" : "启动时自动检查更新";
  return <>
    {settings && <section className="update-settings" aria-label="软件更新"><div><h2>软件更新</h2><p>当前版本 {state.currentVersion}</p><p role="status">{status}</p></div><button className="button secondary" disabled={!desktop || active || installing || blocked} onClick={() => void check()}>{state.phase === "ready" ? "安装更新" : "检查更新"}</button></section>}
    <AlertDialog.Root open={open} onOpenChange={value => { if (!installing) { setOpen(value); if (!value) setDismissed(state.version); } }}>
      <AlertDialog.Portal><AlertDialog.Overlay className="dialog-overlay" /><AlertDialog.Content className="dialog-content">
        <AlertDialog.Title className="dialog-title">新版本已准备好</AlertDialog.Title>
        <AlertDialog.Description className="dialog-description">WinGlow {state.version} 已下载，安装后会自动重新打开软件。</AlertDialog.Description>
        <div className="dialog-actions"><AlertDialog.Cancel asChild><button className="button secondary" disabled={installing}>稍后</button></AlertDialog.Cancel><button className="button primary" disabled={blocked || installing} onClick={() => void install()}>{installing ? "正在安装…" : "更新并重启"}</button></div>
        {error && <p className="notice error" role="alert">{error}</p>}
      </AlertDialog.Content></AlertDialog.Portal>
    </AlertDialog.Root>
    {settings && error && !open && <p className="notice error" role="alert">{error}</p>}
  </>;
}
