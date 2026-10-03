import { useState } from "react";
import { ChevronRight, File, Folder, Menu, RefreshCw } from "lucide-react";
import { cn } from "../lib/cn";
import type { MenuItem } from "../types";

const locations = ["桌面", "文件夹", "文件夹空白处", "所有文件", "EXE 程序", "图片", "文本", "PDF", "压缩文件", "音视频", "快捷方式", "磁盘"];
const levelLabel = (item: MenuItem) => item.menuLevel === "extension" ? "扩展菜单" : item.menuLevel === "cascade" ? "一级 → 二级" : "一级菜单";
function MenuIcon({ item }: { item: MenuItem }) {
  return item.iconDataUrl ? <img className="menu-item-icon" src={item.iconDataUrl} width={22} height={22} alt="" title={item.iconSource ?? undefined} /> : <Menu className="menu-item-icon placeholder" size={22} aria-hidden="true" />;
}
function ExampleMenu({ item, enabled, target }: { item: MenuItem; enabled: boolean; target: string }) {
  const object = target === "所有文件" || target === "文件与文件夹" ? "文件" : target;
  const background = object === "桌面" || object === "文件夹空白处";
  return <div className="context-example" aria-label={enabled ? "开启时的菜单示意" : "关闭后的菜单示意"}>
    <div className="context-object">{object === "文件夹" ? <Folder size={14} /> : <File size={14} />} 右键{object}</div>
    <div className="context-entry muted">{background ? "刷新" : "打开"}</div>
    {enabled && <div className="context-entry highlighted"><MenuIcon item={item} /><span>{item.label}{item.menuLevel === "extension" && "（扩展）"}</span>{item.menuLevel === "cascade" && <ChevronRight size={13} />}</div>}
    {enabled && item.menuLevel === "cascade" && item.children.length > 0 && <div className="context-submenu">{item.children.slice(0, 4).map((label, index) => <div key={`${index}-${label}`}>{label}</div>)}{item.children.length > 4 && <div>…</div>}</div>}
    <div className="context-entry muted">{object === "桌面" ? "个性化" : "属性"}</div>
  </div>;
}
export function MenuManager({ items, disabled, loading, onRefresh, onToggle }: {
  items: MenuItem[]; disabled: boolean; loading: boolean;
  onRefresh: () => void; onToggle: (id: string, enabled: boolean) => void;
}) {
  const [search, setSearch] = useState("");
  const [location, setLocation] = useState(() => items.some(item => item.targets.includes("桌面")) ? "桌面" : "");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const shown = items.filter(item => (!location || item.targets.includes(location)) &&
    [item.label, item.rawLabel, item.group, ...item.targets, ...item.children].join(" ").toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));
  const selected = shown.find(item => item.id === selectedId) ?? shown[0];
  const availableLocations = locations.filter(target => items.some(item => item.targets.includes(target)));
  return <div className="menu-manager">
    <div className="menu-toolbar">
      <select aria-label="右键位置" value={location} onChange={event => setLocation(event.target.value)}><option value="">全部注册项</option>{availableLocations.map(target => <option key={target} value={target}>{target}</option>)}</select>
      <input type="search" aria-label="查找菜单项" placeholder="查找名称或子菜单" value={search} onChange={event => setSearch(event.target.value)} />
      <span className="menu-count">{shown.length} 项</span>
      <button className="icon-button" aria-label="刷新菜单项" disabled={loading} onClick={onRefresh}><RefreshCw size={16} /></button>
    </div>
    <div className="menu-layout">
      <div className="menu-list" aria-label="菜单项">
        {shown.length === 0 ? <p className="empty-state">{search || location ? "没有匹配的菜单项" : "没有可管理的菜单项"}</p> : shown.map(item => <div key={item.id} className={cn("menu-row", item.id === selected?.id && "selected")}>
          <button className="menu-select" aria-label={`查看 ${item.label}`} aria-pressed={item.id === selected?.id} onClick={() => setSelectedId(item.id)} title={`${item.rawLabel} · ${item.targets.join("、")}`}>
            <MenuIcon item={item} /><span className="menu-row-copy"><span className="menu-row-name">{item.label}</span><span className="menu-row-meta">{item.group === "扩展菜单" ? (item.targets.includes("所有文件") ? "文件等" : item.targets.slice(0, 2).join("、")) : item.group} · {levelLabel(item)}</span></span>
          </button>
          <label className="switch menu-switch"><input type="checkbox" role="switch" aria-label={`${item.label} 显示在右键菜单`} checked={item.enabled} disabled={disabled} onChange={event => { setSelectedId(item.id); onToggle(item.id, event.target.checked); }} /><span aria-hidden="true" /></label>
        </div>)}
      </div>
      {selected && <aside className="menu-detail" aria-label="菜单开关效果">
        <div className="menu-detail-heading"><MenuIcon item={selected} /><h2>{selected.label}</h2></div>
        <p className="menu-detail-location">右键位置：{selected.targets.includes("所有文件") ? (selected.targets.includes("文件夹") ? "各种文件、文件夹" : "各种文件（包括 EXE、图片等）") : selected.targets.join("、")}</p>
        <div className="menu-comparison"><section><h3>开启：显示</h3><ExampleMenu item={selected} enabled target={location || selected.targets[0]} /></section><section><h3>关闭：隐藏</h3><ExampleMenu item={selected} enabled={false} target={location || selected.targets[0]} /></section></div>
        <p className="menu-preview-note">开关效果示意。Windows 11 部分项目在“显示更多选项”中。</p>
        {selected.visibilityNote && <p className="menu-detail-explanation">{selected.visibilityNote}</p>}
        {selected.menuLevel === "extension" ? <p className="menu-detail-explanation">这是程序提供的一组扩展菜单。关闭会隐藏该扩展在多个位置提供的菜单；内部子项由程序动态生成，无法仅从注册表确定。</p> : selected.menuLevel === "cascade" ? <div className="menu-detail-explanation"><p>关闭会隐藏整个入口和它下面的二级菜单。</p>{selected.children.length > 0 ? <><h3>下面的二级菜单</h3><ul>{selected.children.map((label, index) => <li key={`${index}-${label}`}>{label}</li>)}</ul></> : <p>子项由程序动态生成，需在实际右键菜单中查看。</p>}</div> : <p className="menu-detail-explanation">关闭后，这个直接显示的菜单项会消失。</p>}
        <details className="menu-source"><summary>原始名称与范围</summary><p>{selected.rawLabel}</p><p>{selected.kind} · {levelLabel(selected)}</p>{selected.iconSource && <p>图标：{selected.iconSource}</p>}</details>
      </aside>}
    </div>
  </div>;
}
