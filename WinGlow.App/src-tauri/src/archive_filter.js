import { menu_controller } from "mshell";

// Only remove existing archive actions; never create or re-enable menu entries.
export function compactArchiveMenu(menu) {
  const entries = menu.items.map(item => ({ item, data: item.data() }));
  const clean = value => String(value ?? "").replace(/\([&＆][^)]*\)|[&＆]/g, "").trim().toLowerCase();
  const actions = entries.map(entry => ({ ...entry, name: clean(entry.data.name) }));
  const compress = actions.filter(e => typeof e.data.submenu !== "function" && /压缩|add to archive|compress/.test(e.name) && !/解压|decompress|uncompress|extract/.test(e.name));
  const extract = actions.filter(e => typeof e.data.submenu !== "function" && /解压|extract|unzip/.test(e.name));
  const preferredCompress = compress.find(e => /^(添加到压缩文件|压缩文件|add to archive)/.test(e.name)) ?? compress[0];
  const preferredExtract = extract.find(e => /^(解压文件|解压到|extract files|unzip to)/.test(e.name) && !/当前|每个|here|each|\%/.test(e.name)) ?? extract[0];
  for (const entry of [...compress, ...extract]) {
    if (entry !== preferredCompress && entry !== preferredExtract) entry.item.remove();
  }
  for (const entry of actions) {
    if (/^(使用|用|open with).*360.*(压缩|zip)/.test(entry.name)) entry.item.remove();
    if (typeof entry.data.submenu === "function" && !compress.includes(entry) && !extract.includes(entry)) {
      const original = entry.data.submenu;
      entry.item.update_data({ submenu: child => { original(child); compactArchiveMenu(child); } });
    }
  }
}
menu_controller.add_menu_listener(e => compactArchiveMenu(e.menu));
