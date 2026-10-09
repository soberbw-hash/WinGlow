import { useState } from "react";
import type { Appearance } from "../types";

export function VisualSettings({ id, settings, enabled, disabled, onApply }: {
  id: "window-material" | "start-menu"; settings: Appearance; enabled: boolean; disabled: boolean;
  onApply: (settings: Appearance) => void;
}) {
  const [draft, setDraft] = useState(settings);
  const changed = id === "window-material" ? draft.material !== settings.material : draft.tint !== settings.tint || draft.radius !== settings.radius;
  return <fieldset className="visual-controls" disabled={disabled}>
    <legend className="sr-only">{id === "window-material" ? "窗口背景设置" : "开始菜单外观设置"}</legend>
    {id === "window-material" ? <label className="visual-control">背景材质<select value={draft.material} onChange={event => setDraft({ ...draft, material: Number(event.target.value) })}><option value={3}>亚克力 · 磨砂</option><option value={2}>云母 · 柔和底色</option></select></label> : <>
      <label className="visual-control">背景浓度 <output>{draft.tint}%</output><input aria-label="开始菜单背景浓度" type="range" min={20} max={95} value={draft.tint} onChange={event => setDraft({ ...draft, tint: Number(event.target.value) })} /></label>
      <label className="visual-control">圆角 <output>{draft.radius}</output><input aria-label="开始菜单圆角" type="range" min={0} max={24} value={draft.radius} onChange={event => setDraft({ ...draft, radius: Number(event.target.value) })} /></label>
    </>}
    <button className="button secondary" disabled={disabled || enabled && !changed} onClick={() => onApply(draft)}>{enabled ? "应用设置" : "启用并应用"}</button>
  </fieldset>;
}
