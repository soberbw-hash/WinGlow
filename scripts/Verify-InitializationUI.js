// A failed component resume remains visible and retryable. No native writes.
async (page) => {
  const errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(()=>{
    window.isTauri=true;
    window.initializationTest={attempts:0};
    window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener() {}};
    window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>1,unregisterCallback() {},invoke:async command=>{
      if(command.startsWith('plugin:event|'))return 1;
      if(command==='optimization_upgrade_status')return {running:false,finished:true,message:'桌面初始化未完成：壁纸组件未就绪',error:true};
      if(command==='load_bootstrap')return {presets:[],activePresetId:null,activeFontLabel:'Windows 默认',currentPreviewFamily:'sans-serif',currentPreviewWeight:400,needsFontRefresh:false,backupDir:'',canRestore:false,pendingRecovery:null};
      if(command==='load_shell')return {items:[],tweaks:[],optionalTools:[],appearance:{material:3,tint:20,radius:10},taskbarSupported:true,startMenuSupported:true,windowMaterialSupported:true,optimizationActive:false,optimizationPending:false,pendingRecovery:false};
      if(command==='load_desktop_settings')return {taskbar:{desktop:{accent:'blur',color:'#000000',opacity:0,showLine:true},maximized:null,startOpen:null},classic:{taskbar:0,alignment:1,combine:0,smallIcons:false,startMenu:0},wallpaper:{items:[],selected:'',intervalMinutes:5,volume:0,canRestore:false}};
      if(command==='update_status')return {currentVersion:'1.1.10',phase:'current',version:null,downloaded:0,total:null,error:null};
      if(command==='retry_optimization_upgrade') {
        if(++window.initializationTest.attempts===1)throw '壁纸组件仍未就绪，请重试。';
        return {message:'桌面初始化已完成，当前优化方案已是最新。'};
      }
      throw Error('Unexpected IPC: '+command);
    }};
  });
  await page.goto('http://127.0.0.1:1420/');
  await page.getByText('桌面初始化未完成：壁纸组件未就绪',{exact:true}).waitFor();
  await page.getByRole('button',{name:'设置与还原',exact:true}).click();
  const retry=page.getByRole('button',{name:'重试初始化',exact:true});
  await retry.click();
  await page.getByText('壁纸组件仍未就绪，请重试。',{exact:true}).waitFor();
  if(!await retry.isEnabled())throw Error('Failed initialization cannot retry');
  await retry.click();
  await page.getByText('桌面初始化已完成，当前优化方案已是最新。',{exact:true}).waitFor();
  await retry.waitFor({state:'hidden'});
  if(await page.evaluate(()=>window.initializationTest.attempts)!==2)throw Error('Wrong number of retries');
  if(errors.length)throw Error(errors.join('\n'));
  return {failureVisible:true,retryAfterFailure:true,recovered:true,errors};
}
