import { readFileSync } from "node:fs";
import vm from "node:vm";
import assert from "node:assert/strict";
const code = readFileSync(new URL("../WinGlow.App/src-tauri/src/archive_filter.js", import.meta.url), "utf8").replace(/import .*;\n/, "").replace("export function", "function");
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
