// test_frontend_modules.cjs v1.0.75
const assert=require('node:assert/strict');
const {chromium}=require('/root/google-images-local/node_modules/playwright');
(async()=>{
  const browser=await chromium.launch({headless:true,executablePath:'/root/.cache/ms-playwright/chromium-1228/chrome-linux64/chrome',args:['--no-sandbox']});
  try {
    const page=await browser.newPage({viewport:{width:1440,height:960}});
    const errors=[],requests=[];page.on('pageerror',error=>errors.push(error.message));
    const entry={id:'fixture-chat',name:'Fixture',bytes:1024,modified_ns:'1000000000'};
    const connection={entry,source:'master',target:'fixture-peer',operation:'sync',scopes:{chat:true,paths:true,apps:true},flow:{phase:'ready',initialized:true,requested:false}};
    const state={device_runtime:{'fixture-peer':{available:true,active:true,build:{version:'v1.0.0',source_id:'fixture-old'}}},build:{version:'v1.0.1',source_id:'fixture-master'},role:'master',paused:false,checks_paused:true,settings_paused:false,devices:[{id:'fixture-peer',name:'Peer'}],chats:[entry],ssh_settings:{},appearance:{'device:master':{label:'ROG',icon:'desktop'}},chat_settings:{},links:[connection],positions:{master:{x:220,y:160},'fixture-peer':{x:720,y:550}},link_positions:{},connections:{'fixture-peer':'online'},job:{status:'idle'},rules:[]};
    const status={...state,analyses:{'master:fixture-chat':{status:'ready',manifest:{paths:['/home/project']}}},app_checks:{'master:fixture-chat':{status:'ready',requirements:[{label:'Python 3'}]}},preparations:{},transfers:{},discovery:{peers:[],ignored:0},shared:{devices:state.devices,appearance:state.appearance,chat_settings:state.chat_settings,links:state.links,positions:state.positions,link_positions:{}}};
    await page.route('**/api/**',async route=>{
      const endpoint=new URL(route.request().url()).pathname;requests.push(endpoint);
      let response;
      if(endpoint==='/api/state')response=state;
      else if(endpoint==='/api/status')response=status;
      else if(endpoint==='/api/peers')response=status.discovery;
      else if(endpoint==='/api/catalog')response={root:'/fixture',chats:[entry]};
      else if(endpoint==='/api/terminal')response={screen:'Console fixture'};
      else if(endpoint==='/api/links/play'){connection.flow.requested=true;response=state.links;}
      else if(endpoint==='/api/device/control'){const body=route.request().postDataJSON();response={active:body.action==='toggle-awake',build:{version:'v1.0.0',source_id:'fixture-old'},update:body.action==='update'?{status:'running'}:null};}
      else if(endpoint==='/api/check')response=state.connections;
      else if(endpoint==='/api/positions')response={ok:true};
      else throw Error('Unexpected API '+endpoint);
      await route.fulfill({json:response});
    });
    await page.goto('http://localhost:23232');
    await page.waitForSelector('.map-node');
    assert.equal(await page.locator('.map-node').count(),2);
    assert.equal(await page.locator('.chat-link').count(),1);
    assert.equal(await page.locator('.node-restart').count(),2);
    await page.waitForFunction(()=>document.querySelector('.node-version')?.dataset.state==='outdated');
    assert.equal(await page.locator('.node-version').getAttribute('title'),'v1.0.0');
    assert.equal(await page.locator('.map-node[data-id="fixture-peer"] .node-toggle-awake').getAttribute('aria-pressed'),'true');
    assert.equal(await page.locator('.map-node[data-id="fixture-peer"] .node-toggle-awake').getAttribute('title'),'Standby bloccato');
    assert.equal(await page.locator('.node-version').textContent(),'');
    assert.equal(await page.locator('.node-version svg').count(),1);
    await page.locator('.node-version').click();
    await page.waitForFunction(()=>document.querySelector('.node-version')?.dataset.state==='updating');
    await page.locator('.map-node[data-id="master"] .node-toggle-awake').click();
    await page.waitForFunction(()=>document.querySelector('.map-node[data-id="master"] .node-toggle-awake').getAttribute('aria-pressed')==='true');
    assert.equal(await page.locator('#panel').isVisible(),false);
    for(const asset of ['panels.js','map.js','console.js'])assert.equal(await page.evaluate(asset=>!!document.querySelector('script[src="/'+asset+'"]'),asset),true);
    await page.locator('.map-node[data-id="master"]').click();
    await page.locator('.library-row > .icon-button').click();
    await page.locator('.chat-editor summary').filter({hasText:'Sincronizza'}).click();
    assert.equal(await page.locator('.chat-editor [data-refresh]').count(),3);
    await page.locator('.chat-editor .popover-close').click();
    await page.locator('#close').click();
    await page.locator('.link-open').click();
    await page.locator('.terminal-destinations [data-device-id="master"]').click();
    await page.waitForFunction(()=>document.querySelector('.terminal-screen')?.textContent==='Console fixture');
    await page.locator('.chat-terminal .popover-close').click();
    await page.locator('.link-play').click();
    await page.waitForFunction(()=>document.querySelector('.link-play')?.disabled===true);
    assert(requests.includes('/api/links/play'));assert.equal(requests.filter(path=>path==='/api/device/control').length,3,'only explicit clicks use device control; automatic status uses the unified flow');assert.deepEqual(errors,[]);
    console.log('Split frontend: assets load, map/library/context editor/console/Play work together; APIs mocked, no real writes.');
  }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
