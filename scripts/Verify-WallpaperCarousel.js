async (page) => {
  await page.goto('http://127.0.0.1:1420/src-tauri/wallpaper/index.html');
  const results=[];
  for(const [width,height] of [[2560,1440],[3840,2160]]) {
    await page.setViewportSize({width,height});
    const samples=await page.evaluate(()=>{
      const scenes=[...document.querySelectorAll('.scene')];
      return [0,300000,600000].map(time=>{
        for(const scene of scenes) scene.getAnimations()[0].currentTime=time;
        const opacities=scenes.map(scene=>Number(getComputedStyle(scene).opacity));
        return {time,opacities,winner:opacities.indexOf(Math.max(...opacities))};
      });
    });
    if(new Set(samples.map(s=>s.winner)).size!==3) throw Error('Carousel did not show all three scenes');
    const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth||document.documentElement.scrollHeight>innerHeight);
    if(overflow) throw Error('Wallpaper overflow');
    results.push({width,height,samples,overflow});
  }
  return results;
}
