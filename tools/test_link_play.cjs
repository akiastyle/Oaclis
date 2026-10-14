// test_link_play.cjs v1.1.30
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('/root/google-images-local/node_modules/playwright');
(async () => {
  const app = fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8');
  const snippet = app.slice(app.indexOf('  const play = marker.querySelector(".link-play")'), app.indexOf('  main.querySelector(".chat-badge").style.color'));
  const browser = await chromium.launch({ headless: true, executablePath: '/root/.cache/ms-playwright/chromium-1228/chrome-linux64/chrome', args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage();
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    const css = fs.readFileSync(path.join(__dirname, '../ui/badge-motion.css'), 'utf8');
    await page.setContent(`<style>:root{--scope-active:#3f8e58;--duration-ui:.5s;--radius-icon:50%;--panel:#222;--glass-opacity:60%;--glass-blur:3px;--space-8:8px}.motion-badge{position:absolute;left:100px;top:100px;width:44px;height:44px}${css}</style><div class="motion-badge" data-motion-phase="idle"><button class="link-main">C</button><button class="link-edit">M</button><button class="link-open">T</button></div>`);
    await page.evaluate(snippet => {
      window.marker=document.querySelector('.motion-badge');window.connection={entry:{id:'chat'},operation:'sync',flow:{initialized:true,phase:'ready'}};
      window.data={paused:false};window.chatLinks=[];window.calls=0;
      const render = new Function('marker','connection','data','name','guard','api','renderLink','main','edit','open','existing',snippet);
      window.draw=()=>render(marker,connection,data,'Chat',fn=>fn,async (route,value)=>{if(route!=='links/play')throw Error(route);calls++;return [{...value,flow:{...value.flow,requested:true}}];},()=>{marker.querySelector('.link-play').disabled=chatLinks[0]?.flow.requested===true;},marker.querySelector('.link-main'),marker.querySelector('.link-edit'),marker.querySelector('.link-open'),true);
      draw();
    }, snippet);
    assert.equal(await page.locator('.link-play').isVisible(),true);
    assert.equal(await page.locator('.link-play').evaluate(el=>getComputedStyle(el).color),'rgb(63, 142, 88)');
    await page.locator('.link-play').click();
    await page.waitForFunction(()=>calls===1 && document.querySelector('.link-play').disabled);
    await page.evaluate(()=>{connection.flow.phase='aligned';draw();});assert.equal(await page.locator('.link-play').isVisible(),false);
    await page.evaluate(()=>{connection.operation='send';connection.flow.phase='ready';draw();});assert.equal(await page.locator('.link-play').isVisible(),false);
    assert.deepEqual(errors,[]);
    console.log('Play: visible and green for changes, queues once, hidden after alignment or completed one-way transfer.');
  } finally { await browser.close(); }
})().catch(error=>{console.error(error);process.exitCode=1;});
