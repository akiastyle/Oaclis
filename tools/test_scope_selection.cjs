// Read-only browser regression: polling must not discard selected details.
const assert = require('node:assert/strict');
const { chromium } = require(process.env.OACLIS_PLAYWRIGHT);
(async () => {
  const browser = await chromium.launch({headless:true, executablePath:process.env.OACLIS_CHROMIUM, args:['--no-sandbox']});
  try {
    const page = await browser.newPage();
    await page.route('**/api/positions', route => route.fulfill({json:{ok:true}}));
    await page.goto('http://localhost:23232');
    await page.waitForSelector('.map-node');
    const result = await page.evaluate(() => {
      const entry = {id:'selection-fixture', name:'Prova', analysis:{status:'ready', manifest:{paths:['/home/project'], apps:['Dipendenza progetto']}}};
      editChat(entry, document.querySelector('.map-node'));
      const panel = document.querySelector('.chat-editor');
      panel.querySelector('.scope-tools').closest('details').open = true;
      const box = panel.querySelector('[data-detail="paths"]');
      const node = box.firstChild;
      const selection = window.getSelection(), range = document.createRange();
      range.selectNodeContents(node); selection.removeAllRanges(); selection.addRange(range);
      for (let i=0; i<4; i++) renderScopeDetails(panel,entry);
      const stable = selection.toString() === '/home/project' && box.firstChild === node;
      entry.analysis.manifest.paths = ['/home/updated'];
      renderScopeDetails(panel,entry);
      const deferred = selection.toString() === '/home/project' && box.firstChild === node;
      selection.removeAllRanges(); renderScopeDetails(panel,entry);
      const updated = box.textContent === '/home/updated';
      closeChatEditor(); return {stable,deferred,updated};
    });
    assert.deepEqual(result,{stable:true,deferred:true,updated:true});
    console.log('Selected details survive polling; changed details update after selection ends: OK');
  } finally { await browser.close(); }
})().catch(error=>{console.error(error);process.exitCode=1;});
