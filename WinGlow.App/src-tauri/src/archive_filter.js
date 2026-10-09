import { menu_controller, fs, breeze } from "mshell";

// Rebuilt from OFF switches in the same transaction as the registry changes.
const hiddenRules = /*WinGlowRules*/ { exact: [], patterns: [] };
const normalize = value => String(value ?? "").split("\t")[0].replace(/\([&＆][^)]*\)|[&＆]|[.。…\s]/g, "").toLowerCase();
const enabledNames = new Set((hiddenRules.enabled ?? []).map(normalize));
const hiddenNames = new Set(hiddenRules.exact.map(normalize));
const hiddenPatterns = hiddenRules.patterns.map(pattern => new RegExp(pattern, "i"));
function hideDisabled(menu) {
  for (const item of menu.items) {
    const data = item.data();
    const names = [data.name, data.origin_name].map(normalize);
    if (!names.some(name => enabledNames.has(name)) && names.some(name => hiddenNames.has(name) || hiddenPatterns.some(pattern => pattern.test(name)))) {
      item.remove();
    } else if (typeof data.submenu === "function") {
      const original = data.submenu;
      item.update_data({ submenu: child => { original(child); hideDisabled(child); } });
    }
  }
}

// Only remove existing archive actions; never create or re-enable menu entries.
export function compactArchiveMenu(menu) {
  const entries = menu.items.map(item => ({ item, data: item.data() }));
  const clean = value => String(value ?? "").replace(/\([&＆][^)]*\)|[&＆]/g, "").trim().toLowerCase();
  const actions = entries.map(entry => ({ ...entry, name: clean(entry.data.name) }));
  const compress = actions.filter(e => typeof e.data.submenu !== "function" && /压缩|壓縮|add to (?:archive|["“])|添加到.*\.(?:zip|7z|rar)|compress/.test(e.name) && !/解压|解壓|decompress|uncompress|extract|unzip/.test(e.name));
  const extract = actions.filter(e => typeof e.data.submenu !== "function" && /解压|解壓|extract|unzip/.test(e.name));
  const preferredCompress = compress.find(e => /^(添加到压缩(?:文件|包)|添加到壓縮|压缩文件|add to archive)/.test(e.name)) ?? compress[0];
  const preferredExtract = extract.find(e => /^(解压文件|解压到|解壓|extract files|extract to|unzip to)/.test(e.name) && !/当前|每个|here|each|\%/.test(e.name)) ?? extract[0];
  for (const entry of [...compress, ...extract]) {
    if (entry !== preferredCompress && entry !== preferredExtract) entry.item.remove();
  }
  for (const entry of actions) {
    if (/^(使用|用|open with|open archive).*?(?:360.*(?:压缩|zip)|7-zip|winrar|czip)/.test(entry.name)) entry.item.remove();
    // 360/CZIP's "other compression commands" only duplicates existing creation
    // commands. Do not leave an empty expandable row in the folder menu.
    if (/其他压缩命令|other compression commands/.test(entry.name) && preferredCompress) {
      entry.item.remove();
      continue;
    }
    if (typeof entry.data.submenu === "function" && !compress.includes(entry) && !extract.includes(entry)) {
      const original = entry.data.submenu;
      // Each installed provider keeps its own basic actions. Sharing the parent's
      // selection could empty another provider's entire submenu on folders.
      entry.item.update_data({ submenu: child => { original(child); compactArchiveMenu(child); } });
    }
  }
}
menu_controller.add_menu_listener(e => {
  const before = e.menu.items.map(item => item.data().name);
  hideDisabled(e.menu);
  compactArchiveMenu(e.menu);
  // One bounded diagnostic snapshot of the actual rendered menu, no file paths.
  try { fs.write(breeze.data_directory() + "/WinGlow-last-menu.json", JSON.stringify({ before, after: e.menu.items.map(item => item.data().name) })); } catch {}
});
