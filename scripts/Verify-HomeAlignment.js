// Browser-only IPC fixture. Run via playwright-cli run-code --filename.
async (page) => {
  await page.addInitScript(() => {
    window.isTauri = true;
    const active = location.search.includes('optimized');
    window.__TAURI_INTERNALS__ = { invoke: async command => {
      if (command === 'load_bootstrap') return { presets: [], activePresetId: null, activeFontLabel: 'Windows 默认', currentPreviewFamily: 'sans-serif', currentPreviewWeight: 400, needsFontRefresh: false, backupDir: '', canRestore: false, pendingRecovery: null };
      if (command === 'load_shell') return { items: [], tweaks: [], optionalTools: [], appearance: { material: 3, tint: 20, radius: 10 }, taskbarSupported: true, startMenuSupported: true, windowMaterialSupported: true, optimizationActive: active, optimizationPending: false, pendingRecovery: false };
      if (command === 'update_status') return { currentVersion: '1.1.6', phase: 'idle', version: null, downloaded: 0, total: null, error: null };
      throw Error('Unexpected IPC: ' + command);
    }};
  });
  const results = [];
  for (const [width, height] of [[1080, 740], [1707, 960], [1920, 1080], [760, 600]]) {
    await page.setViewportSize({width, height});
    for (const active of [false, true]) {
      await page.goto('http://127.0.0.1:1420/' + (active ? '?optimized' : ''));
      await page.getByRole('button', {name: active ? '已优化' : '一键优化', exact: true}).last().waitFor();
      await page.evaluate(() => document.fonts.ready);
      const geometry = await page.evaluate(() => {
        const center = selector => { const r = document.querySelector(selector).getBoundingClientRect(); return r.x + r.width / 2; };
        const buttons = [...document.querySelectorAll('.home-actions > button')].map(button => { const r = button.getBoundingClientRect(); return {width:r.width,height:r.height}; });
        return { centers: ['.home-intro img','.home-intro h1','.home-actions'].map(center), buttons, overflow: document.documentElement.scrollWidth > innerWidth };
      });
      if (geometry.overflow || Math.max(...geometry.centers) - Math.min(...geometry.centers) > 1) throw Error('Homepage alignment failed at '+width);
      if (active && (geometry.buttons.length !== 2 || Math.abs(geometry.buttons[0].width - geometry.buttons[1].width) > 1 || Math.abs(geometry.buttons[0].height - geometry.buttons[1].height) > 1)) throw Error('Buttons differ in size');
      results.push({width,height,active,...geometry});
      await page.locator('.home-settings > summary').click();
      for (const label of ['透明任务栏', '窗口背景', '开始菜单美化']) {
        if (await page.getByRole('switch', {name:label, exact:true}).count() !== 1) throw Error('Missing home setting: ' + label);
      }
      if (await page.getByRole('slider').count() !== 2) throw Error('Start menu customization missing');
      if (width === 1707 && active) await page.screenshot({path:'output/playwright/home-settings-1.1.6.png'});
      await page.getByRole('button', {name:'基础美化', exact:true}).click();
      for (const label of ['透明任务栏', '窗口背景', '开始菜单美化']) {
        if (await page.getByRole('switch', {name:label, exact:true}).count() !== 1) throw Error('Missing basic customization setting: ' + label);
      }
    }
  }
  return results;
}
