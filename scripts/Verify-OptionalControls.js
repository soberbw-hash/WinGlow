// Browser-only IPC fixture; does not modify the host.
async (page) => {
  await page.addInitScript(() => {
    window.isTauri = true;
    window.__testCalls = []; window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener:()=>{}};
    window.__TAURI_INTERNALS__ = {metadata:{currentWindow:{label:"main"},currentWebview:{label:"main"}},transformCallback:()=>1,unregisterCallback:()=>{},invoke: async (command, args) => {
      if(command === 'load_bootstrap') return {presets:[],activePresetId:null,activeFontLabel:'Windows 默认',currentPreviewFamily:'sans-serif',currentPreviewWeight:400,needsFontRefresh:false,backupDir:'',canRestore:false,pendingRecovery:null};
      if(command === 'load_shell') return {items:[],tweaks:[],optionalTools:[{id:'lively',label:'动态壁纸',installed:true,managed:true,supported:true,note:'选择动态壁纸'},{id:'explorer-patcher',label:'经典布局',installed:true,managed:true,supported:false,note:'当前 Windows 版本尚未验证经典布局。'}],appearance:{material:3,tint:20,radius:10},taskbarSupported:true,startMenuSupported:true,windowMaterialSupported:true,optimizationActive:false,optimizationPending:false,pendingRecovery:false};
      if(command.startsWith('plugin:event|')) return 1;
      if(command === 'update_status') return {currentVersion:'1.1.8',phase:'idle',version:null,downloaded:0,total:null,error:null};
      window.__testCalls.push({command,args});
      return {message:'测试请求已收到'};
    }};
  });
  const results=[];
  for(const [width,height] of [[1080,740],[1707,960],[760,600]]) {
    await page.setViewportSize({width,height});
    await page.goto('http://127.0.0.1:1420/');
    await page.getByRole('button',{name:'基础美化',exact:true}).click();
    for(const name of ['壁纸库','经典布局设置','恢复系统任务栏','选择视频或动图','暂停','继续','静音','停止壁纸']) {
      const button=page.getByRole('button',{name,exact:true});
      await button.waitFor();
      if(!await button.isEnabled()) throw Error('Disabled recovery/control: '+name);
    }
    await page.getByRole('button',{name:'暂停',exact:true}).click();
    await page.getByText('测试请求已收到',{exact:true}).waitFor();
    const calls=await page.evaluate(()=>window.__testCalls);
    if(!calls.some(c=>c.command==='wallpaper_control'&&c.args.action==='pause')) throw Error('Missing wallpaper IPC');
    const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth);
    if(overflow) throw Error('Horizontal overflow '+width);
    results.push({width,height,overflow,calls});
  }
  console.log(JSON.stringify(results));
}
