// test_frontend_modules.cjs v1.0.141
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require('/root/google-images-local/node_modules/playwright');
(async()=>{
  const browser=await chromium.launch({headless:true,executablePath:'/root/.cache/ms-playwright/chromium-1228/chrome-linux64/chrome',args:['--no-sandbox']});
  try {
    const page=await browser.newPage({viewport:{width:1440,height:960}});
    const errors=[],requests=[];let deleteFailure=false,releaseDelete=null,holdStatus=false,releaseStatus=null;page.on('pageerror',error=>errors.push(error.message));
    const entry={id:'fixture-chat',name:'Fixture',bytes:1024,modified_ns:'1000000000'};
    const connection={entry,source:'master',target:'fixture-peer',operation:'sync',scopes:{chat:true,paths:true,apps:true},flow:{phase:'ready',initialized:true,requested:false}};
    const state={device_runtime:{'fixture-peer':{available:true,active:true,build:{version:'v1.0.0',source_id:'fixture-old'}}},build:{version:'v1.0.1',source_id:'fixture-master'},role:'master',paused:false,checks_paused:true,settings_paused:false,devices:[{id:'fixture-peer',name:'Peer'}],chats:[entry],ssh_settings:{},appearance:{'device:master':{label:'ROG',icon:'desktop'}},chat_settings:{},links:[connection],positions:{master:{x:220,y:160},'fixture-peer':{x:720,y:550}},link_positions:{},connections:{'fixture-peer':'online'},job:{status:'idle'},rules:[]};
    const status={...state,analyses:{'master:fixture-chat':{status:'ready',manifest:{paths:['/home/project']}}},app_checks:{'master:fixture-chat':{status:'ready',requirements:[{label:'Python 3'}]}},preparations:{},transfers:{},discovery:{peers:[],ignored:0},shared:{devices:state.devices,appearance:state.appearance,chat_settings:state.chat_settings,links:state.links,positions:state.positions,link_positions:{}}};
    await page.route(/\/(?:app|panels|map|console)\.js(?:\?.*)?$/, route=>route.fulfill({contentType:'text/javascript',body:fs.readFileSync(path.join(__dirname,'../ui',new URL(route.request().url()).pathname),'utf8')}));
    await page.route('**/api/**',async route=>{
      const endpoint=new URL(route.request().url()).pathname;requests.push(endpoint);
      let response;
      if(endpoint==='/api/state')response=state;
      else if(endpoint==='/api/status'){response=structuredClone(status);if(holdStatus){holdStatus=false;await new Promise(resolve=>{releaseStatus=resolve;});}}
      else if(endpoint==='/api/peers')response=status.discovery;
      else if(endpoint==='/api/catalog')response={root:'/fixture',chats:[entry]};
      else if(endpoint==='/api/scopes'){const body=route.request().postDataJSON();state.chat_settings[entry.id]={scopes:body.scopes};connection.scopes=body.scopes;response={preferences:state.chat_settings,links:state.links};}
      else if(endpoint==='/api/analyze')response=status.analyses['master:fixture-chat'];
      else if(endpoint==='/api/app-check')response=status.app_checks['master:fixture-chat'];
      else if(endpoint==='/api/terminal')response={screen:'Console fixture'};
      else if(endpoint==='/api/links'){const body=route.request().postDataJSON();state.links=[...state.links,{...body,flow:{phase:'checking'}}];status.shared.links=state.links;response=state.links;}
      else if(endpoint==='/api/links/delete'){await new Promise(resolve=>{releaseDelete=resolve;});if(deleteFailure)return route.fulfill({status:503,json:{error:'database is locked'}});state.links=state.links.filter(value=>value.entry.id!==route.request().postDataJSON().entry.id);status.shared.links=state.links;response=state.links;}
      else if(endpoint==='/api/links/play'){connection.flow.requested=true;response=state.links;}
      else if(endpoint==='/api/device/control'){const body=route.request().postDataJSON();const previous=state.device_runtime[body.device_id]||{build:state.build};response={...previous,active:body.action==='toggle-awake'?!previous.active:previous.active,update:body.action==='update'?{status:'running'}:previous.update,observed_at:Date.now()};state.device_runtime[body.device_id]=response;}
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
    await page.locator('.chat-editor [data-scope="paths"]').click();
    assert.equal(await page.locator('.chat-editor [data-scope="paths"]').getAttribute('aria-pressed'),'false');
    assert.equal(await page.locator('.chat-editor [data-scope="apps"]').getAttribute('aria-pressed'),'true');
    assert.equal(await page.locator('.chat-editor [data-scope="chat"]').getAttribute('aria-pressed'),'true');
    const appRequests=requests.filter(p=>p==='/api/app-check').length;
    await page.locator('.chat-editor [data-refresh="paths"]').click();
    await page.waitForFunction(()=>!document.querySelector('.chat-editor [data-refresh="paths"]').disabled);
    assert.equal(requests.filter(p=>p==='/api/app-check').length,appRequests,'path refresh must not launch app detection');

    await page.locator('.chat-editor .popover-close').click();
    await page.locator('#close').click();
    await page.locator('.link-open').click();
    await page.locator('.terminal-destinations [data-device-id="master"]').click();
    await page.waitForFunction(()=>document.querySelector('.terminal-screen')?.textContent==='Console fixture');
    await page.locator('.chat-terminal .popover-close').click();
    await page.locator('.link-play').click();
    await page.waitForFunction(()=>document.querySelector('.link-play')?.disabled===true);
    assert(requests.includes('/api/links/play'));assert.equal(requests.filter(path=>path==='/api/device/control').length,3,'only explicit clicks use device control; automatic status uses the unified flow');assert.deepEqual(errors,[]);
    await page.evaluate(()=>{window.fixtureMapNode=document.querySelector('.map-node[data-id="master"]');});
    connection.flow.phase='checking';
    await page.evaluate(()=>poll());
    assert(await page.evaluate(()=>document.querySelector('.map-node[data-id="master"]')===window.fixtureMapNode),'link state changes must preserve device nodes');
    holdStatus=true;releaseStatus=null;
    await page.evaluate(()=>{window.staleLinkPoll=poll();});
    while(!releaseStatus)await new Promise(resolve=>setTimeout(resolve,10));
    await page.locator('.link-main').hover();
    await page.evaluate(()=>{sourceId='master';return selectChat({id:'second-fixture',name:'Second fixture'},'fixture-peer','sync');});
    assert.equal(await page.locator('.chat-link').count(),2,'a saved link must appear even while another badge is hovered');
    releaseStatus();await page.evaluate(()=>window.staleLinkPoll);
    await page.mouse.move(1300,900);await page.evaluate(()=>renderLink(true));
    assert.equal(await page.locator('.chat-link').count(),2,'an older status response must not hide the newly saved link');
    await page.evaluate(()=>poll());
    assert(await page.evaluate(()=>document.querySelector('.map-node[data-id="master"]')===window.fixtureMapNode),'adding links must not rebuild device nodes');
    status.shared.positions={...state.positions,master:{x:260,y:160}};
    await page.mouse.move(1300,900);
    await page.evaluate(()=>poll());
    assert(await page.evaluate(()=>document.querySelector('.map-node[data-id="master"]')!==window.fixtureMapNode),'real device position changes still update the map');
    page.on('dialog',dialog=>dialog.accept());
    await page.evaluate(()=>{window.connectionToDelete=chatLinks.find(value=>value.entry.id==='second-fixture');window.deleteEditor=editChat(connectionToDelete.entry,document.querySelector('.link-edit'),'master',connectionToDelete);window.deleteRequest=removeConnection(connectionToDelete,deleteEditor).then(()=>null,error=>error.message);});
    await page.waitForFunction(()=>!window.deleteEditor.isConnected);
    assert.equal(await page.locator('.chat-link').count(),1,'arc and editor close before the delete request finishes');
    while(!releaseDelete)await new Promise(resolve=>setTimeout(resolve,10));
    deleteFailure=true;releaseDelete();
    assert.match(await page.evaluate(()=>window.deleteRequest),/Collegamento ripristinato/);
    assert.equal(await page.locator('.chat-link').count(),2,'failed deletion restores the association');
    assert(await page.evaluate(()=>!window.deleteEditor.isConnected),'failed deletion must not leave the editor open');
    deleteFailure=false;releaseDelete=null;holdStatus=true;releaseStatus=null;
    await page.evaluate(()=>{window.staleDeletePoll=poll();});
    while(!releaseStatus)await new Promise(resolve=>setTimeout(resolve,10));
    await page.evaluate(()=>{window.deleteEditor=editChat(connectionToDelete.entry,document.querySelector('.link-edit'),'master',connectionToDelete);window.deleteRequest=removeConnection(connectionToDelete,deleteEditor).then(()=>null,error=>error.message);});
    await page.waitForFunction(()=>!window.deleteEditor.isConnected);
    while(!releaseDelete)await new Promise(resolve=>setTimeout(resolve,10));
    releaseDelete();
    assert.equal(await page.evaluate(()=>window.deleteRequest),null);
    releaseStatus();await page.evaluate(()=>window.staleDeletePoll);
    await page.evaluate(()=>renderLink(true));
    assert.equal(await page.locator('.chat-link').count(),1);
    console.log('Split frontend: assets load, map/library/context editor/console/Play work together; APIs mocked, no real writes.');
  }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
