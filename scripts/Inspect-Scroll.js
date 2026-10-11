async (page) => {
  return await page.evaluate(() => {
    const rect = selector => { const e=document.querySelector(selector); return {rect:e.getBoundingClientRect().toJSON(),scroll:e.scrollHeight,client:e.clientHeight,css:getComputedStyle(e).overflow}; };
    return {height:innerHeight,root:rect('html'),body:rect('body'),app:rect('.app-shell'),workspace:rect('.workspace'),main:rect('main')};
  });
}
