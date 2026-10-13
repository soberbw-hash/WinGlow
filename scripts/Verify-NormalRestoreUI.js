// IPC fixture only; does not reset this computer.
async page => {
  await page.addInitScript(() => {
    window.isTauri = true;
    window.restoreProbe = { calls: [], active: true };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
      transformCallback: () => 1, unregisterCallback() {},
      invoke: async (command, args) => {
        if (command.startsWith("plugin:event|")) return 1;
        if (command === "optimization_upgrade_status") return { running: false, finished: true, message: "", error: false };
        if (command === "update_status") return { currentVersion: "1.1.14", phase: "current", downloaded: 0, total: null };
        if (command === "load_bootstrap") return { presets: [], activePresetId: null, activeFontLabel: "Windows 默认", currentPreviewFamily: "sans-serif", currentPreviewWeight: 400, needsFontRefresh: false, backupDir: "", canRestore: true, pendingRecovery: null };
        if (command === "load_shell") return { items: [], tweaks: [], optionalTools: [{ id: "lively", label: "动态壁纸", installed: true, enabled: true, managed: true, supported: true, note: "手动选择" }], appearance: { material: 3, tint: 20, radius: 10 }, taskbarSupported: true, startMenuSupported: true, windowMaterialSupported: true, optimizationActive: window.restoreProbe.active, optimizationPending: false, pendingRecovery: false };
        if (command === "load_desktop_settings") return { taskbar: { desktop: { accent: "blur", color: "#000000", opacity: 0, showLine: true }, maximized: null, startOpen: null }, wallpaper: { items: [{ id: "builtin:carousel", title: "自动轮播", preview: null, scene: null }, { id: "library:rain", title: "Rain", preview: null, scene: null }], selected: "library:rain", intervalMinutes: 5, volume: 0, canRestore: true } };
        if (command === "open_homepage") { window.restoreProbe.links ??= []; window.restoreProbe.links.push(args.kind); return; }
        window.restoreProbe.calls.push(command);
        if (command === "restore_normal") { window.restoreProbe.active = false; return { message: "普通还原完成" }; }
        throw Error("Unexpected IPC: " + command);
      }
    };
  });
  await page.goto("http://127.0.0.1:1421/");
  await page.getByRole("button", { name: "基础美化", exact: true }).click();
  if (await page.getByRole("button", { name: "自动轮播", exact: true }).count()) throw Error("Legacy rotation card remains");
  if (await page.getByLabel("切换间隔", { exact: true }).count()) throw Error("Rotation interval remains");
  await page.getByRole("button", { name: "设置与还原", exact: true }).click();
  await page.getByRole("button", { name: "作者主页 ↗", exact: true }).click();
  await page.getByRole("button", { name: "项目主页 ↗", exact: true }).click();
  if (JSON.stringify(await page.evaluate(() => window.restoreProbe.links)) !== JSON.stringify(["author", "project"])) throw Error("Homepage links did not dispatch the right target");
  if (await page.getByText("菜单识别库", { exact: true }).count()) throw Error("Standalone dictionary settings remain");
  const restore = page.getByRole("button", { name: "一键还原", exact: true });
  await restore.click();
  await page.getByRole("button", { name: "取消", exact: true }).click();
  if ((await page.evaluate(() => window.restoreProbe.calls)).length) throw Error("Cancelled restore changed state");
  await restore.click();
  await page.getByRole("button", { name: "继续", exact: true }).click();
  await page.getByText("普通还原完成", { exact: true }).waitFor();
  const calls = await page.evaluate(() => window.restoreProbe.calls);
  if (JSON.stringify(calls) !== JSON.stringify(["restore_normal"])) throw Error("Restore did not use the single ordinary command");
  await page.getByRole("navigation").getByRole("button", { name: "一键优化", exact: true }).click();
  if (!await page.locator(".home-actions").getByRole("button", { name: "一键优化", exact: true }).isEnabled()) throw Error("Restore did not clear optimized state");
  return { ordinaryRestore: true, cancelPreservesState: true, scanRepairNotCalled: true, rotationRemoved: true };
}
