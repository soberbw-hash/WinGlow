import { readFileSync } from "node:fs";
import vm from "node:vm";
import assert from "node:assert/strict";
const code = readFileSync(new URL("../WinGlow.App/src-tauri/src/archive_filter.js", import.meta.url), "utf8").replace(/import .*;\r?\n/, "").replace("export function", "function");
let listener;
vm.runInNewContext(code, { menu_controller: { add_menu_listener: fn => { listener = fn; } } });
function menu(names) {
  return { items: names.map(name => ({ value: { name }, removed: false, data() { return this.value; }, remove() { this.removed = true; }, update_data(data) { Object.assign(this.value, data); } })) };
}
const native = menu(["添加到压缩文件(&A)...", "压缩为 test.zip", "压缩并 E-mail", "解压文件(&F)...", "解压到当前文件夹", "解压每个文件到单独文件夹", "使用360压缩打开", "复制", "属性"]);
listener({ menu: native });
assert.deepEqual(native.items.filter(i => !i.removed).map(i => i.value.name), ["添加到压缩文件(&A)...", "解压文件(&F)...", "复制", "属性"]);
const empty = menu(["属性"]); listener({ menu: empty });
assert.equal(empty.items.length, 1); assert.equal(empty.items[0].removed, false);
const nested = menu(["360 压缩"]);
const children = menu(["Add to archive...", "Compress and email", "Extract files...", "Extract here"]);
nested.items[0].value.submenu = () => {};
listener({ menu: nested }); nested.items[0].value.submenu(children);
assert.deepEqual(children.items.filter(i => !i.removed).map(i => i.value.name), ["Add to archive...", "Extract files..."]);
console.log("Archive filtering passed: native actions preserved, extra actions hidden, nested menu supported, no actions added.");
for (const [provider, labels] of [
  ["7-Zip", ["Add to archive...", 'Add to "sample.7z"', 'Add to "sample.zip"', "Compress and email", "Extract files...", "Extract here", 'Extract to "sample\\"']],
  ["WinRAR", ["添加到压缩包...", '添加到 "sample.rar"', "压缩并邮件", "解压到...", "解压到当前文件夹", '解压到 "sample\\"']],
  ["CZIP", ["添加到压缩文件...", '添加到 "sample.zip"', "解压到...", "解压到当前文件夹"]],
]) {
  const parent = menu([provider]); const child = menu(labels);
  parent.items[0].value.submenu = () => {};
  listener({ menu: parent }); parent.items[0].value.submenu(child);
  assert.deepEqual(child.items.filter(i => !i.removed).map(i => i.value.name), [labels[0], labels.find(l => /^(Extract files|解压到\.\.\.)/.test(l))], provider);
}
const folder = menu(["添加到压缩文件...", '添加到 "folder.zip"', "其他压缩命令", "复制"]);
folder.items[2].value.submenu = () => {};
listener({ menu: folder });
assert.deepEqual(folder.items.filter(i => !i.removed).map(i => i.value.name), ["添加到压缩文件...", "复制"]);
console.log("360/CZIP, 7-Zip and WinRAR keep compression/extraction; folder duplicate submenu removed.");
const mixed = menu(["添加到压缩文件...", "7-Zip"]);
const sevenZipFolder = menu(["Add to archive...", 'Add to "folder.7z"', 'Add to "folder.zip"']);
mixed.items[1].value.submenu = () => {};
listener({ menu: mixed }); mixed.items[1].value.submenu(sevenZipFolder);
assert.deepEqual(sevenZipFolder.items.filter(i => !i.removed).map(i => i.value.name), ["Add to archive..."]);
console.log("Multiple installed providers retain their basic folder action without empty submenus.");

function visibilityListener(rules) {
  let callback;
  const script = code.replace("/*WinGlowRules*/ { exact: [], patterns: [] }", JSON.stringify(rules));
  vm.runInNewContext(script, { menu_controller: { add_menu_listener: fn => { callback = fn; } }, fs: { write() {} }, breeze: { data_directory: () => "." } });
  return callback;
}
const off = visibilityListener({ exact: ["用 WorkBuddy 打开", "使用微信输入法隔空传送"], patterns: ["quark|夸克", "defender", "百度网盘", "^自动备份(?:该|此)?文件夹$"] });
const desktopMenu = menu(["打开(&O)", "用 WorkBuddy 打开\tW", "上传到夸克网盘", "自动备份该文件夹", "使用微信输入法隔空传送", "使用 Microsoft Defender扫描...", "上传到百度网盘", "添加到压缩文件(&A)...", "复制(&C)", "属性(&R)"]);
off({ menu: desktopMenu });
assert.deepEqual(desktopMenu.items.filter(item => !item.removed).map(item => item.value.name), ["打开(&O)", "添加到压缩文件(&A)...", "复制(&C)", "属性(&R)"]);
const on = visibilityListener({ exact: [], patterns: [] });
const manuallyEnabled = menu(["用 WorkBuddy 打开", "上传到夸克网盘"]);
on({ menu: manuallyEnabled });
assert.ok(manuallyEnabled.items.every(item => !item.removed));
const lazyParent = menu(["工具"]), lazyChild = menu(["用 WorkBuddy 打开", "复制"]);
lazyParent.items[0].value.submenu = () => {};
off({ menu: lazyParent }); lazyParent.items[0].value.submenu(lazyChild);
assert.deepEqual(lazyChild.items.filter(item => !item.removed).map(item => item.value.name), ["复制"]);
console.log("OFF intent also filters extended/disabled verbs and lazy children; manual ON remains possible.");
const collision = visibilityListener({ exact: ["Open  Project(&P)...", "用 WorkBuddy 打开"], patterns: ["workbuddy"], enabled: ["OPEN PROJECT", "用 WorkBuddy 打开(&W)"] });
const collisionMenu = menu(["Open Project\tP", "用 WorkBuddy 打开", "属性"]);
collision({ menu: collisionMenu });
assert.ok(collisionMenu.items.every(item => !item.removed), "An enabled equivalent title must survive normalization and provider patterns");
console.log("Enabled title protection covers casing, whitespace, accelerator labels and provider patterns.");
