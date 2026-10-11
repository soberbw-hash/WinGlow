// Browser-only IPC fixture; native rendering is verified separately on the desktop.
async (page) => {
  await page.addInitScript(() => {
    window.isTauri=true;
    const look={accent:'blur',color:'#000000',opacity:0,showLine:true};
    let settings={taskbar:{desktop:look,maximized:null,startOpen:null},wallpaper:{items:[{id:'builtin:carousel',title:'自动轮播',preview:null,scene:null},...Array.from({length:12},(_,i)=>({id:'library:project'+i,title:'壁纸 '+i,preview:null,scene:null}))],selected:'builtin:carousel',intervalMinutes:5,volume:0,canRestore:true}};
    let wallpaperEnabled=true;
    window.__testCalls=[];
    window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
    window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(command,args)=>{
      if(command==='load_desktop_settings')return structuredClone(settings);
      if(command==='optimization_upgrade_status')return {running:false,finished:true,message:'',error:false};
      if(command==='load_bootstrap')return {presets:[],activePresetId:null,activeFontLabel:'Windows 默认',currentPreviewFamily:'sans-serif',currentPreviewWeight:400,needsFontRefresh:false,backupDir:'',canRestore:false,pendingRecovery:null};
      if(command==='load_shell')return {items:[],tweaks:[],optionalTools:[{id:'lively',label:'动态壁纸',installed:true,enabled:wallpaperEnabled,managed:true,supported:true,note:'点击图片切换'}],appearance:{material:3,tint:20,radius:10},taskbarEnabled:true,taskbarSupported:true,startMenuSupported:true,windowMaterialSupported:true,optimizationActive:false,optimizationPending:false,pendingRecovery:false};
      if(command==='update_status')return {currentVersion:'1.1.12',phase:'idle',version:null,downloaded:0,total:null,error:null};
      if(command.startsWith('plugin:event|'))return 1;
      window.__testCalls.push({command,args});
      if(command==='optional_tool')wallpaperEnabled=args.action==='enable';
      if(command==='apply_taskbar_settings')settings.taskbar=args.settings;
      if(command==='select_wallpaper')settings.wallpaper.selected=args.id;
      return {message:'内部设置已应用'};
    }};
  });
  const results=[];
  for(const [width,height] of [[1024,720],[1707,960],[2560,1440],[3840,2160]]){
    await page.setViewportSize({width,height});await page.goto('http://127.0.0.1:1420');
    await page.getByRole('button',{name:'基础美化',exact:true}).click();
    const wallpaperSwitch=page.getByRole('switch',{name:'动态壁纸',exact:true});
    if(!await wallpaperSwitch.isChecked())throw Error('Wallpaper switch must show actual active state');
    await wallpaperSwitch.click();
    await page.waitForFunction(()=>document.querySelector('[role="switch"][aria-label="动态壁纸"]')?.checked===false);
    await wallpaperSwitch.click();
    await page.waitForFunction(()=>document.querySelector('[role="switch"][aria-label="动态壁纸"]')?.checked===true);
    await page.getByRole('button',{name:'壁纸 1',exact:true}).click();
    await page.getByText('内部设置已应用',{exact:true}).waitFor();
    if(!await page.getByRole('button',{name:'壁纸 1',exact:true}).getAttribute('aria-pressed'))throw Error('Missing selected state');
    for(const name of ['应用壁纸','应用音量','暂停','继续','静音','停止壁纸','应用经典布局设置'])if(await page.getByRole('button',{name,exact:true}).count())throw Error('Removed control remains: '+name);
    if(await page.getByText('经典布局',{exact:true}).count())throw Error('Classic layout still present');
    const taskbar=page.getByRole('group',{name:'任务栏外观设置',exact:true});
    await taskbar.getByLabel('常用场景材质',{exact:true}).selectOption('clear');
    await page.getByRole('button',{name:'应用任务栏设置',exact:true}).click();
    await page.waitForFunction(()=>window.__testCalls.some(c=>c.command==='apply_taskbar_settings'&&c.args.settings.desktop.accent==='clear'));
    const calls=await page.evaluate(()=>window.__testCalls);
    if(!calls.some(c=>c.command==='select_wallpaper'&&c.args.id==='library:project1'))throw Error('Click did not apply wallpaper');
    for(const action of ['enable','disable'])if(!calls.some(c=>c.command==='optional_tool'&&c.args.action===action))throw Error('Switch did not send '+action);
    const scroll=await page.evaluate(()=>{
      const main=document.querySelector('main');main.scrollTop=main.scrollHeight;
      return {overflow:document.documentElement.scrollWidth>innerWidth,root:document.documentElement.scrollHeight,body:document.body.scrollHeight,height:innerHeight,mainBottom:main.getBoundingClientRect().bottom,appBottom:document.querySelector('.app-shell').getBoundingClientRect().bottom,nested:Array.from(main.querySelectorAll('*')).filter(e=>['auto','scroll'].includes(getComputedStyle(e).overflowY)&&e.scrollHeight>e.clientHeight+1).length};
    });
    if(scroll.overflow||scroll.root>height+1||scroll.body>height+1||Math.abs(scroll.mainBottom-height)>1||Math.abs(scroll.appBottom-height)>1||scroll.nested)throw Error(JSON.stringify(scroll));
    results.push({width,height,scroll});
  }
  return {results,removedControls:true,directSelection:true};
}
