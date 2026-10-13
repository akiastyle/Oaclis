// UI fixtures only; no real settings, chat transfers, or console commands.
const assert = require('node:assert/strict');
const { chromium } = require(process.env.OACLIS_PLAYWRIGHT);
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.OACLIS_CHROMIUM,args:['--no-sandbox']});
 try {
  const page=await browser.newPage({viewport:{width:1280,height:800}});
  const chats=['one','two'].map(id=>({id,name:id,analysis:{status:'ready',manifest:{paths:['/home/'+id]}}}));
  await page.route('**/api/positions',r=>r.fulfill({json:{ok:true}}));
  await page.route('**/api/catalog',r=>r.fulfill({json:{chats}}));
  await page.route('**/api/status',r=>r.fulfill({json:{job:{status:'idle'},connections:{},analyses:{}}}));
  await page.goto('http://localhost:23232');await page.waitForSelector('.map-node');
  await page.evaluate(()=>openDevice('master'));await page.waitForSelector('.library-row');
  assert.equal(await page.locator('#panel').evaluate(p=>getComputedStyle(p).getPropertyValue('--glass-opacity').trim()),'60%');
  await page.waitForTimeout(550);
  const bar=await page.locator('#panel-device').boundingBox();
  const before=await page.locator('#panel').boundingBox();
  await page.mouse.move(bar.x+bar.width/2,bar.y+bar.height/2);await page.mouse.down();
  await page.mouse.move(bar.x+bar.width/2-180,bar.y+bar.height/2+60,{steps:5});await page.mouse.up();
  await page.waitForFunction(()=>{const p=document.querySelector('#panel'),h=p.querySelector('.panel-bar').getBoundingClientRect();return Math.abs(Number(p.tether.querySelector('line').getAttribute('y2'))-(h.top+h.height/2))<1;});
  const after=await page.locator('#panel').boundingBox();assert(after.x<before.x-100);assert(after.y>before.y+30);
  const result=await page.evaluate(chats=>{
    const anchor=document.querySelector('.map-node');
    const first=editChat(chats[0],anchor), second=editChat(chats[1],anchor);
    const count=document.querySelectorAll('.chat-editor').length;
    const unique=first.id!==second.id;
    const reused=editChat(chats[0],anchor)===first && document.querySelectorAll('.chat-editor').length===2;
    closeChatEditor(first);
    const independent=second.isConnected && document.querySelectorAll('.chat-editor').length===1;
    return {count,unique,reused,independent};
  },chats);
  assert.deepEqual(result,{count:2,unique:true,reused:true,independent:true});
  await page.waitForFunction(()=>[...document.querySelectorAll('.chat-editor')].every(p=>{
    const h=p.querySelector('h2').getBoundingClientRect();
    return Math.abs(Number(p.tether.querySelector('line').getAttribute('y2'))-(h.top+h.height/2))<1;
  }));
  await page.evaluate(()=>{closeChatEditor();closePanel();});
  await page.waitForFunction(()=>document.querySelectorAll('.panel-tether').length===0);
  console.log('Glass library drag, independent editors, and title-level tethers: OK');
 } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
