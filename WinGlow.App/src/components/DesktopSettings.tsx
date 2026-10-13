import { useState } from "react";
import { cn } from "../lib/cn";
import { applyTaskbarSettings,selectWallpaper,chooseWallpaper,wallpaperControl } from "../lib/tauri";
import type { ActionResult,TaskbarSettings,TaskbarLook,DesktopSettings } from "../types";
type Run = (operation:()=>Promise<ActionResult>)=>Promise<void>;

function TaskbarEditor({value,onChange,label}:{value:TaskbarLook;onChange:(v:TaskbarLook)=>void;label:string}) {
  return <div className="desktop-control-row">
    <label className="visual-control">{label}材质<select aria-label={`${label}材质`} value={value.accent} onChange={e=>onChange({...value,accent:e.target.value})}><option value="normal">系统默认</option><option value="blur">模糊</option><option value="clear">透明</option><option value="acrylic">亚克力</option><option value="opaque">不透明</option></select></label>
    <label className="visual-control compact-color">{label}颜色<input aria-label={`${label}颜色`} type="color" value={value.color} onChange={e=>onChange({...value,color:e.target.value})}/></label>
    <label className="visual-control">{label}颜色浓度<output>{value.opacity}%</output><input aria-label={`${label}颜色浓度`} type="range" min={0} max={100} value={value.opacity} onChange={e=>onChange({...value,opacity:Number(e.target.value)})}/></label>
    <label className="desktop-check"><input type="checkbox" checked={value.showLine} onChange={e=>onChange({...value,showLine:e.target.checked})}/>显示任务栏细线</label>
  </div>;
}
export function TaskbarControls({settings,disabled,run}:{settings:TaskbarSettings;disabled:boolean;run:Run}) {
  const [draft,setDraft]=useState(settings);
  const changed=JSON.stringify(draft)!==JSON.stringify(settings);
  return <fieldset className="desktop-settings" disabled={disabled}><legend className="sr-only">任务栏外观设置</legend>
    <TaskbarEditor value={draft.desktop} label="常用场景" onChange={desktop=>setDraft({...draft,desktop})}/>
    {([ ["maximized","窗口最大化时"],["startOpen","打开开始菜单时"] ] as const).map(([key,label])=><div className="taskbar-context" key={key}>
      <label className="desktop-check"><input type="checkbox" checked={draft[key]!==null} onChange={e=>setDraft({...draft,[key]:e.target.checked?{...draft.desktop}:null})}/>{label}使用不同效果</label>
      {draft[key] && <TaskbarEditor value={draft[key]} label={label} onChange={value=>setDraft({...draft,[key]:value})}/>}
    </div>)}
    <button className="button secondary" disabled={disabled||!changed} onClick={()=>void run(()=>applyTaskbarSettings(draft))}>应用任务栏设置</button>
  </fieldset>;
}
export function WallpaperControls({settings,disabled,run}:{settings:DesktopSettings["wallpaper"];disabled:boolean;run:Run}) {
  const selected=settings.selected;
  return <fieldset className="desktop-settings wallpaper-settings" disabled={disabled}><legend className="sr-only">壁纸设置</legend>
    <div className="wallpaper-grid" role="group" aria-label="选择壁纸">{settings.items.filter(item=>item.id!=="builtin:carousel").map(item=><button key={item.id} type="button" aria-pressed={selected===item.id} className={cn("wallpaper-card",selected===item.id&&"selected")} onClick={()=>void run(()=>selectWallpaper(item.id,5))}>
      {item.preview?<img src={item.preview} alt="" loading="lazy"/>:<span className={cn("wallpaper-preview",`wallpaper-${item.scene??"carousel"}`)} aria-hidden="true"/>}<span>{item.title}</span>
    </button>)}</div>
    <div className="desktop-control-row">


      <button className="button secondary" onClick={()=>void run(chooseWallpaper)}>导入视频或动图</button>
      <button className="button secondary" disabled={disabled||!settings.canRestore} onClick={()=>void run(()=>wallpaperControl("restore-settings"))}>恢复上次壁纸</button>
    </div>
  </fieldset>;
}
