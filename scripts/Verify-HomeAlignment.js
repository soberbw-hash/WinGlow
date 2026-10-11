// Browser-only IPC fixture. Run via playwright-cli run-code --filename.
async (page) => {
  await page.addInitScript(() => {
    window.isTauri = true;
    const active = location.search.includes('optimized');
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener:()=>{}}; window.__TAURI_INTERNALS__ = {metadata:{currentWindow:{label:"main"},currentWebview:{label:"main"}},transformCallback:()=>1,unregisterCallback:()=>{}, invoke: async command => {
      if (command === 'optimization_upgrade_status') return {running:false,finished:true,message:'',error:false};
      if (command.startsWith('plugin:event|')) return 1;
      if (command === 'load_bootstrap') return { presets: [], activePresetId: null, activeFontLabel: 'Windows 默认', currentPreviewFamily: 'sans-serif', currentPreviewWeight: 400, needsFontRefresh: false, backupDir: '', canRestore: false, pendingRecovery: null };
      if (command === 'load_shell') return { items: [], tweaks: [], optionalTools: [], appearance: { material: 3, tint: 20, radius: 10 }, taskbarSupported: true, startMenuSupported: true, windowMaterialSupported: true, optimizationActive: active, optimizationPending: false, pendingRecovery: false };
      if (command === 'update_status') return { currentVersion: '1.1.6', phase: 'idle', version: null, downloaded: 0, total: null, error: null };
      throw Error('Unexpected IPC: ' + command);
    }};
  });
  const results = [];
  const cases = [
    ...[[1080,740],[760,600]].map(([width,height]) => ({width,height,label:'window'})),
    ...[[2560,1440],[3840,2160]].flatMap(([pixelsX,pixelsY]) =>
      [1,1.25,1.5,1.75,2,2.25,2.5,3].map(scale => ({
        width:Math.round(pixelsX/scale), height:Math.round(pixelsY/scale),
        label:`${pixelsX}x${pixelsY} @ ${scale*100}%`
      })))
  ];
  // CSS pixels are the available logical space after Windows display scaling.
  // This verifies layout; physical monitor moves and native DPI events need device QA.
  for (const {width, height, label} of cases) {
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
      if (await page.locator('.home-settings').count()) throw Error('Homepage details should be removed');
      if (geometry.overflow || Math.max(...geometry.centers) - Math.min(...geometry.centers) > 1) throw Error('Homepage alignment failed at '+width);
      if (active && (geometry.buttons.length !== 2 || Math.abs(geometry.buttons[0].width - geometry.buttons[1].width) > 1 || Math.abs(geometry.buttons[0].height - geometry.buttons[1].height) > 1)) throw Error('Buttons differ in size');
      results.push({label,width,height,active,...geometry});
      await page.getByRole('button', {name:'基础美化', exact:true}).click();
      for (const label of ['透明任务栏', '窗口背景', '开始菜单美化']) {
        if (await page.getByRole('switch', {name:label, exact:true}).count() !== 1) throw Error('Missing basic customization setting: ' + label);
      }
    }
  }
  return results;
}
