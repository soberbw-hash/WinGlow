import { menu_controller, fs, breeze } from "mshell";

// WinGlow can ask the running Breeze runtime to open its native visual settings
// window. Breeze exposes that page only inside its injected JavaScript runtime.
const dataDirectory = breeze.data_directory();
let hiddenRules = { exact: [], patterns: [], enabled: [] };
const normalize = value => String(value ?? "").split("\t")[0].replace(/[\u200B-\u200D\uFEFF]/g, "").replace(/\([&＆][^)]*\)|[&＆]|[.。…\s]/g, "").toLowerCase();
function reloadRules() {
  try {
    const text = fs.read(dataDirectory + "/WinGlow-MenuRules.json");
    if (text.length > 65536) return hiddenRules;
    const rules = JSON.parse(text);
    if (![rules.exact, rules.patterns, rules.enabled ?? []].every(values => Array.isArray(values) && values.every(value => typeof value === "string"))) return hiddenRules;
    for (const pattern of rules.patterns) new RegExp(pattern, "i");
    hiddenRules = rules;
  } catch {}
  return hiddenRules;
}
// Load once and refresh only when WinGlow replaces the rules file. This keeps
// file I/O off the synchronous Explorer menu-construction path.
reloadRules();
try {
  fs.watch(dataDirectory, path => {
    const name = String(path).replaceAll("\\", "/").split("/").pop();
    if (/^WinGlow-MenuRules\.json$/i.test(name ?? "")) {
      reloadRules();
      return;
    }
    const match = /^WinGlow-Open-Config-([0-9a-f-]{36})\.request$/i.exec(name ?? "");
    if (!match) return;
    const requestPath = `${dataDirectory}/${name}`;
    const responsePath = `${requestPath}.done`;
    try {
      const requestId = fs.read(requestPath).trim();
      fs.remove(requestPath);
      if (requestId !== match[1] || typeof globalThis.showConfigPage !== "function") return;
      globalThis.showConfigPage();
      fs.write(responsePath, requestId);
    } catch {}
  });
} catch {}
function hideDisabled(menu, rules) {
  const enabledNames = new Set((rules.enabled ?? []).map(normalize));
  const hiddenNames = new Set(rules.exact.map(normalize));
  const hiddenPatterns = rules.patterns.map(pattern => new RegExp(pattern, "i"));
  for (const item of menu.items) {
    const data = item.data();
    const names = [data.name, data.origin_name].map(normalize);
    if (!names.some(name => enabledNames.has(name)) && names.some(name => hiddenNames.has(name) || hiddenPatterns.some(pattern => pattern.test(name)))) {
      item.remove();
    } else if (typeof data.submenu === "function") {
      const original = data.submenu;
      item.update_data({ submenu: child => { original(child); hideDisabled(child, rules); } });
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
  hideDisabled(e.menu, hiddenRules);
  compactArchiveMenu(e.menu);
});
