// Delayed/error IPC regression; never invokes native mutations.
async (page) => {
  const errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(() => {
    window.isTauri=true;
    const state=window.pollingTest={upgradeCalls:0,updateCalls:0,upgradeActive:0,updateActive:0,upgradeMax:0,updateMax:0,releaseUpgrade:null,releaseUpdate:null};
    window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener() {}};
    window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>1,unregisterCallback() {},invoke:async command=>{
      if(command.startsWith('plugin:event|'))return 1;
      if(command==='load_bootstrap')return {presets:[],activePresetId:null,activeFontLabel:'Windows 默认',currentPreviewFamily:'sans-serif',currentPreviewWeight:400,needsFontRefresh:false,backupDir:'',canRestore:false,pendingRecovery:null};
      if(command==='load_shell')return {items:[],tweaks:[],optionalTools:[],appearance:{material:3,tint:20,radius:10},taskbarSupported:true,startMenuSupported:true,windowMaterialSupported:true,optimizationActive:false,optimizationPending:false,pendingRecovery:false};
      if(command==='load_desktop_settings')return {taskbar:{desktop:{accent:'blur',color:'#000000',opacity:0,showLine:true},maximized:null,startOpen:null},classic:{taskbar:0,alignment:1,combine:0,smallIcons:false,startMenu:0},wallpaper:{items:[],selected:'',intervalMinutes:5,volume:0,canRestore:false}};
      if(command==='optimization_upgrade_status') {
        state.upgradeCalls++;state.upgradeActive++;state.upgradeMax=Math.max(state.upgradeMax,state.upgradeActive);
        try {
          if(state.upgradeCalls===1) {await new Promise(resolve=>state.releaseUpgrade=resolve);throw Error('transient IPC disconnect');}
          return {running:false,finished:true,message:'',error:false};
        } finally {state.upgradeActive--;}
      }
      if(command==='update_status') {
        state.updateCalls++;state.updateActive++;state.updateMax=Math.max(state.updateMax,state.updateActive);
        const first=state.updateCalls===1;
        try {if(first)await new Promise(resolve=>state.releaseUpdate=resolve);return {currentVersion:'1.1.10',phase:first?'error':'current',version:null,downloaded:0,total:null,error:first?'obsolete response':null};}
        finally {state.updateActive--;}
      }
      if(command==='check_updates')return {currentVersion:'1.1.10',phase:'current',version:null,downloaded:0,total:null,error:null};
      throw Error('Unexpected IPC: '+command);
    }};
  });
  await page.goto('http://127.0.0.1:1420/');
  await page.waitForFunction(()=>window.pollingTest.releaseUpgrade&&window.pollingTest.releaseUpdate);
  // Hold the requests across several polling intervals, then reject initialization once.
  await page.waitForTimeout(3300);
  const held=await page.evaluate(()=>({upgrade:window.pollingTest.upgradeCalls,update:window.pollingTest.updateCalls}));
  if(held.upgrade!==1||held.update!==1)throw Error('Overlapping polling: '+JSON.stringify(held));
  const optimize=page.locator('.home-actions').getByRole('button',{name:'一键优化',exact:true});
  if(!await optimize.isDisabled())throw Error('Unknown initialization state allows mutation');
  await page.evaluate(()=>window.pollingTest.releaseUpgrade());
  await page.waitForFunction(()=>window.pollingTest.upgradeCalls===2);
  await optimize.waitFor();
  await page.waitForFunction(()=>!document.querySelector('.home-actions button').disabled);
  await page.getByRole('button',{name:'设置与还原',exact:true}).click();
  await page.getByRole('region',{name:'软件更新',exact:true}).getByRole('button',{name:'检查更新',exact:true}).click();
  await page.getByText('当前版本 1.1.10',{exact:true}).waitFor();
  await page.getByText('已是最新版本',{exact:true}).waitFor();
  await page.evaluate(()=>window.pollingTest.releaseUpdate());
  await page.waitForFunction(()=>window.pollingTest.updateActive===0);
  if(await page.getByText('检查或下载失败，请重试',{exact:true}).count())throw Error('Obsolete status overwrote manual check');
  const result=await page.evaluate(()=>({upgradeCalls:window.pollingTest.upgradeCalls,upgradeMax:window.pollingTest.upgradeMax,updateMax:window.pollingTest.updateMax}));
  if(result.upgradeMax!==1||result.updateMax!==1)throw Error('Concurrent requests');
  if(errors.length)throw Error(errors.join('\n'));
  return {held,result,retryUnlocked:true,staleResponseIgnored:true,errors};
}
