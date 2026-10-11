// Browser IPC acceptance: no registry, wallpaper or runtime changes on the host.
async (page) => {
  await page.addInitScript(() => {
    window.isTauri=true;
    const look={accent:'blur',color:'#000000',opacity:0,showLine:true};
    let settings={taskbar:{desktop:look,maximized:null,startOpen:null},classic:{taskbar:0,alignment:1,combine:0,smallIcons:false,startMenu:0},wallpaper:{items:[{id:'builtin:carousel',title:'自动轮播',preview:null,scene:null},{id:'builtin:night',title:'夜色',preview:null,scene:'night'},{id:'builtin:sea',title:'海色',preview:null,scene:'sea'},{id:'builtin:dawn',title:'晨光',preview:null,scene:'dawn'}],selected:'builtin:carousel',intervalMinutes:5,volume:0,canRestore:true}};
    window.__testCalls=[];
    window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
    window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(command,args)=>{
      if(command==='load_desktop_settings') return structuredClone(settings);
      if(command==='optimization_upgrade_status')return {running:false,finished:true,message:'',error:false};
      if(command==='load_bootstrap')return {presets:[],activePresetId:null,activeFontLabel:'Windows 默认',currentPreviewFamily:'sans-serif',currentPreviewWeight:400,needsFontRefresh:false,backupDir:'',canRestore:false,pendingRecovery:null};
      if(command==='load_shell')return {items:[],tweaks:[],optionalTools:[{id:'lively',label:'动态壁纸',installed:true,managed:true,supported:true,note:'在这里选择壁纸'},{id:'explorer-patcher',label:'经典布局',installed:true,managed:true,supported:location.search.includes('supported'),note:'兼容性检查'}],appearance:{material:3,tint:20,radius:10},taskbarEnabled:true,taskbarSupported:true,startMenuSupported:true,windowMaterialSupported:true,optimizationActive:false,optimizationPending:false,pendingRecovery:false};
      if(command==='update_status')return {currentVersion:'1.1.9',phase:'idle',version:null,downloaded:0,total:null,error:null};
      if(command.startsWith('plugin:event|'))return 1;
      window.__testCalls.push({command,args});
      if(command==='apply_taskbar_settings')settings.taskbar=args.settings;
      if(command==='apply_classic_settings')settings.classic=args.settings;
      if(command==='select_wallpaper')settings.wallpaper.selected=args.id;
      if(command==='set_wallpaper_volume')settings.wallpaper.volume=args.volume;
      return {message:'内部设置已应用'};
    }};
  });
  const results=[];
  for(const [width,height] of [[760,600],[1707,960],[2560,1440]]) {
    await page.setViewportSize({width,height});await page.goto('http://127.0.0.1:1420/');
    await page.getByRole('button',{name:'基础美化',exact:true}).click();
    const taskbar=page.getByRole('group',{name:'任务栏外观设置',exact:true});
    await taskbar.getByLabel('桌面时材质',{exact:true}).selectOption('acrylic');
    await taskbar.getByLabel('窗口最大化时使用不同效果',{exact:true}).check();
    await taskbar.getByLabel('窗口最大化时材质',{exact:true}).selectOption('opaque');
    await page.getByRole('button',{name:'应用任务栏设置',exact:true}).click();
    await page.getByText('内部设置已应用',{exact:true}).waitFor();
    await page.getByRole('button',{name:'海色',exact:true}).click();
    await page.getByRole('button',{name:'海色',exact:true}).getAttribute('aria-pressed');
    const volume=page.getByRole('slider',{name:'壁纸音量',exact:true}); await volume.press('Home'); for(let n=0;n<35;n++)await volume.press('ArrowRight');
    await page.getByRole('button',{name:'应用音量',exact:true}).click();
    await page.getByRole('button',{name:'应用音量',exact:true}).isDisabled();
    const classic=page.getByRole('group',{name:'经典布局设置',exact:true});
    if(!await classic.getByLabel('任务栏布局',{exact:true}).isDisabled())throw Error('Unsupported classic settings exposed');
    if(!await page.getByRole('button',{name:'恢复系统任务栏',exact:true}).isEnabled())throw Error('Recovery blocked');
    if(await page.getByRole('button',{name:'壁纸库',exact:true}).count())throw Error('External wallpaper UI still primary');
    const calls=await page.evaluate(()=>window.__testCalls);
    if(!calls.some(c=>c.command==='apply_taskbar_settings'&&c.args.settings.maximized.accent==='opaque'))throw Error('Missing taskbar config');
    if(!calls.some(c=>c.command==='select_wallpaper'&&c.args.id==='builtin:sea'))throw Error('Missing selection');
    if(!calls.some(c=>c.command==='set_wallpaper_volume'&&c.args.volume===35))throw Error('Missing volume');
    const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth);
    if(overflow)throw Error('Horizontal overflow');
    const scroll = await page.evaluate(() => {
      const main=document.querySelector('main');
      main.scrollTop=main.scrollHeight;
      return {root:document.documentElement.scrollHeight,body:document.body.scrollHeight,height:innerHeight,mainBottom:main.getBoundingClientRect().bottom,appBottom:document.querySelector('.app-shell').getBoundingClientRect().bottom};
    });
    if(scroll.root>scroll.height+1 || scroll.body>scroll.height+1 || Math.abs(scroll.mainBottom-scroll.height)>1 || Math.abs(scroll.appBottom-scroll.height)>1)throw Error(`Outer scrolling or blank area: ${JSON.stringify(scroll)}`);
    results.push({width,height,overflow});
  }
  await page.goto('http://127.0.0.1:1420/?supported');await page.getByRole('button',{name:'基础美化',exact:true}).click();
  const classic=page.getByRole('group',{name:'经典布局设置',exact:true});
  await classic.getByLabel('任务栏布局',{exact:true}).selectOption('2');
  await classic.getByLabel('图标对齐',{exact:true}).selectOption('0');
  await page.getByRole('button',{name:'应用经典布局设置',exact:true}).click();
  await page.getByText('内部设置已应用',{exact:true}).waitFor();
  const calls=await page.evaluate(()=>window.__testCalls);
  if(!calls.some(c=>c.command==='apply_classic_settings'&&c.args.settings.taskbar===2&&c.args.settings.alignment===0))throw Error('Missing classic IPC');
  return {results,classicSupported:true};
}
