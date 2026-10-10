// Run against the Vite preview with playwright-cli run-code --filename.
// IPC exists only in this test page; no Windows setting is changed.
async (page) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    window.isTauri = true;
    const state = window.recoveryTest = { pending: true, optimization: false, mode: 'success', calls: 0, release: null };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => 1, unregisterCallback() {},
      invoke: async command => {
        if (command === 'load_bootstrap') return { presets: [], activePresetId: null, activeFontLabel: 'Windows 默认', currentPreviewFamily: 'sans-serif', currentPreviewWeight: 400, needsFontRefresh: false, backupDir: '', canRestore: false, pendingRecovery: state.pending && !state.optimization ? '上次修改没有完成，请先还原。' : null };
        if (command === 'load_shell') return { items: [], tweaks: [], optionalTools: [], appearance: { material: 3, tint: 78, radius: 16 }, taskbarSupported: true, startMenuSupported: true, windowMaterialSupported: true, optimizationActive: state.optimization, optimizationPending: state.pending && state.optimization, pendingRecovery: state.pending && !state.optimization };
        if (command === 'update_status') return { currentVersion: '1.1.3', phase: 'idle', version: null, downloaded: 0, total: null, error: null };
        if (command.startsWith('plugin:event|')) return 1;
        if (command === 'recover_pending') {
          state.calls++;
          await new Promise(resolve => { state.release = resolve; });
          if (state.mode === 'failure') throw '管理员授权已取消，请重试。';
          state.pending = false;
          return { message: state.mode === 'stale' ? '没有未完成的修改，状态已重新检查。' : '已恢复未完成修改前的设置。' };
        }
        throw Error('Unexpected IPC in recovery test: ' + command);
      }
    };
  });
  const results = [];
  for (const [location, mode, optimization] of [['settings', 'success', false], ['home', 'failure', false], ['home', 'success', true], ['settings', 'stale', false]]) {
    await page.reload();
    await page.evaluate(([mode, optimization]) => { window.recoveryTest.mode = mode; window.recoveryTest.optimization = optimization; }, [mode, optimization]);
    // Ensure the optimization fixture is reflected by the real reload path.
    if (optimization) {
      await page.getByRole('navigation').getByRole('button', { name: '右键菜单', exact: true }).click();
      await page.getByRole('button', { name: '刷新菜单项', exact: true }).click();
      await page.getByRole('navigation').getByRole('button', { name: '一键优化', exact: true }).click();
    }
    if (location === 'settings') await page.getByRole('button', { name: '设置与还原', exact: true }).click();
    const recover = page.getByRole('button', { name: '恢复未完成修改', exact: true });
    await recover.click();
    const progress = page.getByRole('button', { name: '正在恢复…', exact: true });
    await progress.waitFor();
    if (!await progress.isDisabled()) throw Error('Recovery allows repeated clicks');
    if (await page.evaluate(() => window.recoveryTest.calls) !== 1) throw Error('Duplicate recovery request');
    await page.evaluate(() => window.recoveryTest.release());
    if (mode === 'failure') {
      await page.getByText('管理员授权已取消，请重试。', { exact: true }).waitFor();
      if (!await recover.isEnabled()) throw Error('Failed recovery cannot retry');
      await page.evaluate(() => { window.recoveryTest.mode = 'success'; });
      await recover.click();
      await progress.waitFor();
      await page.evaluate(() => window.recoveryTest.release());
    }
    await recover.waitFor({ state: 'hidden' });
    await page.getByText(mode === 'stale' ? '没有未完成的修改，状态已重新检查。' : '已恢复未完成修改前的设置。', { exact: true }).waitFor();
    await page.getByRole('navigation').getByRole('button', { name: '右键菜单', exact: true }).click();
    if (!await page.getByRole('button', { name: '一键精简', exact: true }).isEnabled()) throw Error('Recovery did not unlock editing');
    if (!await page.getByRole('navigation').getByRole('button', { name: '一键优化', exact: true }).count()) throw Error('Navigation disappeared');
    results.push({ location, mode, optimization, passed: true });
  }
  if (errors.length) throw Error(errors.join('\n'));
  console.log(JSON.stringify({ results, errors }));
}
