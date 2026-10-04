import { useState } from "react";
import { ChevronRight, RefreshCw, Terminal, Search, ShieldCheck, LockKeyhole, Cloud, FolderOpen, Image, Music, Settings2, Puzzle, FileText, Monitor, Sparkles } from "lucide-react";
import type { MenuItem } from "../types";

const sections = [
  { label: "常用位置", locations: ["桌面", "所有文件", "文件夹", "文件夹空白处", "磁盘"] },
  { label: "文件类型", locations: ["EXE 程序", "快捷方式", "应用快捷方式", "图片", "文本", "PDF", "压缩文件", "音视频", "未知格式"] },
  { label: "其他位置", locations: ["此电脑", "回收站", "库", "库空白处"] },
];
function MenuIcon({ item }: { item: MenuItem }) {
  if (item.iconDataUrl) return <img className="menu-item-icon" src={item.iconDataUrl} width={22} height={22} alt="" title={item.iconSource ?? undefined} />;
  const label = `${item.label} ${item.rawLabel}`.toLowerCase();
  const Icon = /powershell|命令|terminal|bash/.test(label) ? Terminal : /搜索|search|find/.test(label) ? Search : /bitlocker|解锁|加密/.test(label) ? LockKeyhole : /defender|兼容|扫描/.test(label) ? ShieldCheck : /网盘|quark|cloud|上传/.test(label) ? Cloud : /图片|背景|image|wallpaper/.test(label) ? Image : /media player|播放|音乐/.test(label) ? Music : /chatgpt|豆包|doubao/.test(label) ? Sparkles : /nvidia|显示/.test(label) ? Monitor : /设置|个性化|armoury|asus/.test(label) ? Settings2 : /project|打开|文件夹/.test(label) ? FolderOpen : item.menuLevel === "extension" ? Puzzle : FileText;
  return <Icon className="menu-item-icon semantic-icon" size={22} aria-hidden="true" />;
}
function searchable(item: MenuItem): string {
  return [item.label, item.rawLabel, ...item.children, ...(item.subItems ?? []).map(searchable)].join(" ").toLocaleLowerCase();
}
function MenuRow({ item, depth = 0, disabled, searching, onToggle }: { item: MenuItem; depth?: number; disabled: boolean; searching: boolean; onToggle: (id: string, enabled: boolean) => void }) {
  const [expanded, setExpanded] = useState(false);
  const children = item.subItems ?? [];
  const expandable = children.length > 0;
  const open = expandable && (expanded || searching);
  return <>
    <div className="menu-row" style={{ paddingLeft: depth * 28 }}>
      <button className="menu-select" disabled={!expandable} aria-label={expandable ? `${open ? "收起" : "展开"} ${item.label}` : item.label} aria-expanded={expandable ? open : undefined} onClick={() => setExpanded(!expanded)}>
        <span className="menu-chevron">{expandable && <ChevronRight size={15} style={{ transform: open ? "rotate(90deg)" : undefined }} />}</span><MenuIcon item={item} /><span className="menu-row-copy"><span className="menu-row-name">{item.label}</span>{depth === 0 && <span className="menu-row-meta">{item.targets.includes("所有文件") ? "各种文件" : item.targets.slice(0, 2).join("、")}{item.menuLevel === "extension" && " · 整组开关"}</span>}</span>
      </button>
      <label className="switch menu-switch"><input type="checkbox" role="switch" aria-label={`${item.label} 显示在右键菜单`} checked={item.enabled} disabled={disabled} onChange={event => onToggle(item.id, event.target.checked)} /><span aria-hidden="true" /></label>
    </div>
    {open && children.map(child => <MenuRow key={child.id} item={child} depth={depth + 1} disabled={disabled || !item.enabled} searching={searching} onToggle={onToggle} />)}
  </>;
}
export function MenuManager({ items, disabled, loading, onRefresh, onToggle }: {
  items: MenuItem[]; disabled: boolean; loading: boolean;
  onRefresh: () => void; onToggle: (id: string, enabled: boolean) => void;
}) {
  const [search, setSearch] = useState("");
  const [section, setSection] = useState("常用位置");
  const [location, setLocation] = useState("桌面");
  const [status, setStatus] = useState("all");
  const currentSection = sections.find(s => s.label === section) ?? sections[0];
  const availableLocations = currentSection.locations.filter(target => items.some(item => item.targets.includes(target)));
  const effectiveLocation = availableLocations.includes(location) ? location : "";
  const shown = items.filter(item => (effectiveLocation ? item.targets.includes(effectiveLocation) : item.targets.some(target => currentSection.locations.includes(target))) && (status === "all" || item.enabled === (status === "visible")) && searchable(item).includes(search.trim().toLocaleLowerCase()));
  return <div className="menu-manager">
    <div className="menu-sections" aria-label="菜单分类">{sections.map(s => <button key={s.label} aria-pressed={section === s.label} onClick={() => { setSection(s.label); setLocation(s.locations.find(target => items.some(item => item.targets.includes(target))) ?? ""); }}>{s.label}</button>)}</div>
    <div className="menu-locations" aria-label="右键位置">{["", ...availableLocations].map(target => <button key={target} aria-pressed={effectiveLocation === target} onClick={() => setLocation(target)}>{target || "全部"}</button>)}</div>
    <div className="menu-toolbar">
      <input type="search" aria-label="查找菜单项" placeholder="查找菜单" value={search} onChange={event => setSearch(event.target.value)} />
      <select className="menu-status" aria-label="菜单显示状态" value={status} onChange={event => setStatus(event.target.value)}><option value="all">全部状态</option><option value="visible">正在显示</option><option value="hidden">已隐藏</option></select>
      <span className="menu-count">{shown.length} 项</span><button className="icon-button" aria-label="刷新菜单项" disabled={loading} onClick={onRefresh}><RefreshCw size={16} /></button>
    </div>
    <div className="menu-list" aria-label="菜单项">{shown.length === 0 ? <p className="empty-state">没有匹配的菜单项</p> : shown.map(item => <MenuRow key={item.id} item={item} disabled={disabled} searching={!!search.trim()} onToggle={onToggle} />)}</div>
  </div>;
}
