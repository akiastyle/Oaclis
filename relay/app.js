// app.js v2.128.1233
'use strict';
if('serviceWorker' in navigator)navigator.serviceWorker.register('/sw.js',{scope:'/'}).catch(()=>{});
const app=document.getElementById('app');
window.addEventListener('hashchange',()=>{if(location.hash)location.reload();});
const fragment=new URLSearchParams(location.hash.slice(1));
const authPage=location.pathname.replace(/\/$/,'')==='/auth';
if(location.hash&&!authPage)location.replace('/auth'+location.hash);
sessionStorage.removeItem('oaclis-test-admin');
let invite=fragment.get('pair')||sessionStorage.getItem('oaclis-test-invite')||'';
let code=localStorage.getItem('oaclis-test-code')||'';
history.replaceState(null,'',location.pathname);
if(fragment.get('pair')&&fragment.get('pair')!==sessionStorage.getItem('oaclis-test-invite')){code='';localStorage.removeItem('oaclis-test-code');}
if(invite)sessionStorage.setItem('oaclis-test-invite',invite);
let timer,screenWakeLock=null,wakePending=false;
const voice=OaclisVoice.create(),drafts=new Map();
let authorizedJobs=[],chatJobIndicators=new Map();
let authorizedChats=[],visibleChatList=[],closeConsole=null,consoleWorkspace=null;
const consolePositions=new Map(),imageDrafts=new Map();
function saveDraft(chatId,value){if(value)drafts.set(chatId,value);else drafts.delete(chatId);}
function bindVoice(chat=null){
 const compose=document.querySelector('#console-input textarea'),search=document.getElementById('chat-search');
 let host=document.querySelector('.oaclis-console .voice-controls')||document.querySelector('#chat-tools .voice-controls');
 if(!host){const tools=document.getElementById('chat-tools');if(!tools)return;host=document.createElement('span');host.className='voice-controls';tools.append(host);}
 voice.bind({chatId:chat?.id,compose,fields:()=>[compose,search].filter(Boolean),chats:()=>authorizedChats,list:()=>consoleWorkspace?.list()||visibleChatList,
  view:chat?compose.closest('.oaclis-console'):null,selectChat:selected=>openConsole(selected),
  ...(chat?{splitChat:selected=>openConsole(selected,true),...(consoleWorkspace.split()?{selectPane:position=>consoleWorkspace.selectPane(position)}:{})}:{}),
 },host);
}
async function keepConsoleAwake(){
 if(wakePending||screenWakeLock||document.visibilityState!=='visible'||!document.getElementById('console-screen')||!navigator.wakeLock)return;
 wakePending=true;
 try{
  const lock=await navigator.wakeLock.request('screen');
  if(!document.getElementById('console-screen')){await lock.release();return;}
  screenWakeLock=lock;
  lock.addEventListener('release',()=>{if(screenWakeLock===lock)screenWakeLock=null;});
 }catch{}finally{wakePending=false;}
}
document.addEventListener('visibilitychange',()=>{if(document.visibilityState==='visible')keepConsoleAwake();});
function show(html){clearListJobs();clearTimeout(timer);closeConsole?.();voice.leave();app.innerHTML=html;app.hidden=false;if(screenWakeLock&&!document.getElementById('console-screen')){screenWakeLock.release().catch(()=>{});screenWakeLock=null;}}
function message(text,error=false){let p=document.getElementById('message');if(!p){p=document.createElement('p');p.id='message';p.setAttribute('role','status');app.append(p);}p.className=error?'error':'status';p.textContent=OaclisSettings.text(text);}
async function api(action,body={},signal){
 if(action==='console'){const signed=await OaclisMobileSecurity.sign(code,body);const {image,...rest}=body;body={...rest,signed};}
 const r=await fetch('/auth',{method:'POST',headers:{'Content-Type':'application/json','X-Device-Code':code},body:JSON.stringify({action,...body}),cache:'no-store',signal});
 if(!r.ok)throw Object.assign(new Error(r.status===403?OaclisSettings.text('Accesso non autorizzato.'):OaclisSettings.text('Collegamento temporaneamente non disponibile. Riprova.')),{status:r.status});
 const result=await r.json();
 OaclisMobileSecurity.update(result.security);
 if(action==='console'&&result.request_id!==JSON.parse(body.signed.payload).id)throw Object.assign(new Error(OaclisSettings.text('Collegamento temporaneamente non disponibile. Riprova.')),{status:409});
 return result;
}
function buffer(s){return Uint8Array.from(atob(s.replace(/-/g,'+').replace(/_/g,'/')),c=>c.charCodeAt(0));}
function encode(b){return btoa(String.fromCharCode(...new Uint8Array(b))).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');}
function options(data){const p=data.publicKey;p.challenge=buffer(p.challenge);if(p.user)p.user.id=buffer(p.user.id);for(const c of p.allowCredentials||[])c.id=buffer(c.id);for(const c of p.excludeCredentials||[])c.id=buffer(c.id);return data;}
function credential(c){const r=c.response;const out={id:encode(c.rawId),clientDataJSON:encode(r.clientDataJSON)};for(const k of ['attestationObject','authenticatorData','signature','userHandle'])if(r[k])out[k]=encode(r[k]);return out;}
async function run(button,fn){button.disabled=true;try{await fn();}catch(e){message(e.name==='NotAllowedError'?OaclisSettings.text('Operazione annullata o scaduta. Puoi riprovare.'):e.message,true);}finally{button.disabled=false;}}
async function registerView(){
 if(!window.PublicKeyCredential||!window.isSecureContext)throw new Error(OaclisSettings.text('Apri questa pagina in un browser aggiornato con HTTPS.'));
 if(!code){code=Array.from(crypto.getRandomValues(new Uint8Array(32)),x=>x.toString(16).padStart(2,'0')).join('');localStorage.setItem('oaclis-test-code',code);}
 show(OaclisSettings.html('<h1>Abbina questo telefono</h1><p>Creerai una passkey e un codice casuale per questa installazione. Poi la richiesta dovrà essere approvata dal PC.</p><label for="name">Nome del telefono</label><input id="name" maxlength="60" value="Il mio telefono" autocomplete="off"><button id="register">Crea passkey e richiedi accesso</button>'));
 document.getElementById('register').onclick=e=>run(e.target,async()=>{const name=document.getElementById('name').value.trim();const pairing=await OaclisMobileSecurity.pairing(invite,code,name);const data=await api('register-options',{invite:pairing.invite_hash,name,pairing});const c=await navigator.credentials.create(options(data));await api('register',credential(c));invite='';sessionStorage.removeItem('oaclis-test-invite');await deviceView();});
}
async function consoleResult(chat,request,signal,deadline=Date.now()+23000){
 let result;
 do{result=await api('console-result',{chat_id:chat,request_id:request},signal);}while(result.pending&&Date.now()<deadline);
 return result;
}
function newMobileChat(){
 const base=authorizedChats.find(chat=>chat.id===consoleWorkspace?.active.id)||authorizedChats[0];if(!base)return;
 const existing=document.getElementById('new-chat-dialog');if(existing){existing.querySelector('input').focus();return;}
 const dialog=document.createElement('dialog');dialog.id='new-chat-dialog';dialog.className='voice-dialog new-chat-dialog';dialog.setAttribute('aria-label',OaclisSettings.text('Nuova chat'));
 dialog.innerHTML=OaclisSettings.html('<button type="button" class="voice-close secondary" aria-label="Chiudi">'+OaclisIcon('close')+'</button><h1>Nuova chat</h1><form><label>Nome<input name="name" maxlength="160" value="Nuova chat" required></label>')+OaclisEngines.fields()+OaclisSettings.html('<details><summary>Icone</summary><input type="hidden" name="icon" value="chat"><div class="new-chat-icons" role="group" aria-label="Icone"></div></details><button>Crea chat</button><small role="status"></small></form>');
 const query=async operation=>{if(consoleWorkspace)return consoleWorkspace.query(operation);const request=await api('console',{chat_id:base.id,operation});const result=await consoleResult(base.id,request.request_id);if(result.pending)throw Error('pending');return result;};
 OaclisEngines.bind(dialog.querySelector('form'),async()=>(await query('engines')).engines,()=>query('models'));
 const grid=dialog.querySelector('.new-chat-icons');
 for(const id of Object.keys(icons)){
  const button=document.createElement('button');button.type='button';button.className='secondary';button.innerHTML=OaclisIcon(id);button.title=OaclisIconNames[id];button.setAttribute('aria-label',button.title);button.setAttribute('aria-pressed',String(id==='chat'));
  button.onclick=()=>{dialog.querySelector('[name="icon"]').value=id;for(const item of grid.children)item.setAttribute('aria-pressed',String(item===button));};grid.append(button);
 }
 dialog.querySelector('.voice-close').onclick=()=>dialog.remove();dialog.addEventListener('cancel',event=>{event.preventDefault();dialog.remove();});
 dialog.querySelector('form').onsubmit=async event=>{
  event.preventDefault();const form=event.target,button=form.querySelector('button:not([type="button"])');if(button.disabled)return;button.disabled=true;
  try{
   const values=new FormData(form),text=String(values.get('name')).trim(),icon=String(values.get('icon')),model=String(values.get('model')).trim(),engine=String(values.get('engine'));
   let created;
   if(consoleWorkspace)created=await consoleWorkspace.create(text,icon,model,engine);
   else{const request=await api('console',{chat_id:base.id,operation:'create',text,key:'',icon,model,engine});const result=await consoleResult(base.id,request.request_id);created=result.new_chat;}
   if(!created?.id)throw Error(OaclisSettings.text('Creazione non confermata. Controlla la lista prima di riprovare.'));
   if(!authorizedChats.some(chat=>chat.id===created.id))authorizedChats.push(created);
   dialog.remove();await openConsole(created);
  }catch(error){form.querySelector('[role="status"]').textContent=error.message;}
  finally{button.disabled=false;}
 };
 app.append(dialog);dialog.showModal();dialog.querySelector('[name="name"]').value=OaclisSettings.text('Nuova chat');dialog.querySelector('[name="name"]').select();
}
function clearListJobs(){for(const view of chatJobIndicators.values())view.destroy();chatJobIndicators.clear();}
function updateListJobs(){for(const [chat,view]of chatJobIndicators)view.update(authorizedJobs.filter(job=>job.chat_id===chat));}
function renderAuthorizedChats(chats){
 const create=document.getElementById('new-chat');if(create)create.disabled=!authorizedChats.length;
 const list=document.getElementById('authorized-chats');
 if(!list)return;
 const signature=JSON.stringify(chats);
 if(list.dataset.signature===signature){updateListJobs();return;}
 clearListJobs();list.dataset.signature=signature;list.replaceChildren();
 visibleChatList=chats.slice();
 if(!chats.length){const p=document.createElement('p');p.textContent=OaclisSettings.text('Nessuna chat autorizzata dal master.');list.append(p);return;}
 const ul=document.createElement('ul');
 for(const [index,chat] of chats.entries()){
  const li=document.createElement('li');li.className='authorized-row';
  const progress=document.createElement('span');progress.className='authorized-progress';
  chatJobIndicators.set(chat.id,OaclisJobs.inline(progress));
  li.append(chatButton(chat,index,()=>openConsole(chat)),progress);ul.append(li);
 }
 list.append(ul);updateListJobs();
}
function chatButton(chat,index,select){
 const button=document.createElement('button');button.className='secondary authorized-chat';button.dataset.active=String(chat.active===true);button.setAttribute('aria-label',String(index+1)+'. '+chat.name);
  const number=document.createElement('span');number.className='authorized-number';number.textContent=String(index+1);
  const icon=OaclisTerminal.chatIcon(chat);
  const name=document.createElement('span');name.className='authorized-name';name.textContent=chat.name;
  const play=document.createElement('span');play.className='authorized-play';play.setAttribute('aria-hidden','true');play.innerHTML=OaclisIcon('play');
  button.append(number,icon,name,play);button.onclick=select;return button;
}
async function openConsole(chat,adjacent=false){
 if(consoleWorkspace)return consoleWorkspace.open(chat,adjacent);
 clearTimeout(timer);
 show('');
 const view=OaclisTerminal.createConsole();app.append(view);
 view.querySelector('.console-panes').id='console-panes';view.querySelector('form').id='console-input';view.querySelector('.console-send').id='console-send';view.querySelector('.terminal-keys').id='console-keys';
 const back=document.createElement('button');back.type='button';back.id='back';back.className='secondary';back.title=OaclisSettings.text('Chat autorizzate');back.setAttribute('aria-label',back.title);back.innerHTML=OaclisIcon('chevronLeft');view.querySelector('.terminal-keys').prepend(back);
 const dialog=document.createElement('dialog');dialog.id='console-picker';dialog.className='voice-dialog';dialog.setAttribute('aria-label',OaclisSettings.text('Affianca chat'));app.append(dialog);
 const panes=[],container=document.getElementById('console-panes'),form=document.getElementById('console-input'),text=form.querySelector('textarea'),keys=document.getElementById('console-keys'),picker=document.getElementById('console-picker');
  let active=null,closed=false,pollTimer=null,queue=Promise.resolve(),pending=null,reading=null,inputBusy=false,polling=false;
 const split=document.createElement('button');split.type='button';split.className='console-split secondary';split.innerHTML=OaclisIcon('split');
 const createButton=document.createElement('button');createButton.type='button';createButton.className='console-new secondary';createButton.innerHTML=OaclisIcon('plus');createButton.title=OaclisSettings.text('Nuova chat');createButton.setAttribute('aria-label',createButton.title);createButton.onclick=newMobileChat;
 const fitConsole=()=>{const following=panes.filter(pane=>pane.console.follow);document.documentElement.style.setProperty('--console-height',(window.visualViewport?.height??innerHeight)+'px');requestAnimationFrame(()=>{if(!closed)for(const pane of following)if(!pane.closed){pane.console.follow=true;pane.screen.scrollTop=pane.screen.scrollHeight;}});};
 fitConsole();window.visualViewport?.addEventListener('resize',fitConsole);window.addEventListener('resize',fitConsole);
 function layout(){
  fitConsole();container.dataset.split=String(panes.length===2);panes[0]?.title.append(createButton,split);
  split.setAttribute('aria-pressed',String(panes.length===2));split.title=OaclisSettings.text(panes.length===2?'Chat singola':'Affianca chat');split.setAttribute('aria-label',split.title);
 }
 function remove(pane){saveDraft(pane.chat.id,pane===active?text.value:drafts.get(pane.chat.id)||'');consolePositions.set(pane.chat.id,pane.saved||{top:pane.screen.scrollTop,left:pane.screen.scrollLeft,follow:pane.console.follow});pane.closed=true;pane.console.closed=true;pane.jobsStatus.destroy();pane.element.remove();panes.splice(panes.indexOf(pane),1);}
 function activate(pane){
  if(!pane||pane.closed||closed)return false;
  if(pane===active)return true;
  if(active)saveDraft(active.chat.id,text.value);
  active=pane;view.activeConsole=pane.console;text.value=drafts.get(pane.chat.id)||'';text.setAttribute('aria-label',OaclisSettings.text('Messaggio')+' · '+pane.chat.name);
  for(const item of panes){const selected=item===pane;item.element.classList.toggle('is-active',selected);item.element.setAttribute('aria-current',String(selected));item.title.id=selected?'console-title':'';item.screen.id=selected?'console-screen':'';}
  files.paint();bindVoice(pane.chat);form.querySelectorAll('textarea,button').forEach(element=>element.disabled=!pane.ready||pane.console.preparingImage===true);layout();return true;
 }
 function add(selected,index=panes.length){
  const element=OaclisTerminal.createPane({id:selected.id,name:selected.name,icon:OaclisTerminal.chatIcon(selected)},(action,body)=>action==='stop'?stopConsole(pane):submit(pane,body.key));
  const title=element.querySelector('.console-title'),name=element.querySelector('.console-name');
  const jobs=document.createElement("span");title.append(jobs);const jobsStatus=OaclisJobs.inline(jobs);
  const saved=consolePositions.get(selected.id),pane={chat:selected,element,title,name,jobsStatus,screen:element.querySelector('pre'),console:element.console,ready:false,closed:false,saved};
  panes.splice(index,0,pane);container.insertBefore(element,container.children[index]||null);
  pane.console.follow=saved?.follow??true;pane.console.image=imageDrafts.get(selected.id)||null;
  element.addEventListener('click',event=>{if(!event.target.closest('button'))activate(pane);});element.addEventListener('focusin',event=>{if(!event.target.closest('button'))activate(pane);});
  return pane;
 }
 async function complete(request,signal){
  if(closed)return false;
  const result=await consoleResult(request.pane.chat.id,request.request_id,signal,request.deadline);
  if(closed)return false;
  if(result.pending){pending=null;throw Error(OaclisSettings.text('Collegamento temporaneamente non disponibile. Riprova.'));}
  pending=null;if(result.error)throw Error(result.error);
  if(request.operation==='create')return result.new_chat;
  if(request.operation==='engines'||request.operation==='models')return result;
  const pane=request.pane;if(pane.closed)return true;
  if(request.operation==='stop'||result.stopped===true){
   pane.console.reset();pane.saved=null;
   remove(pane);consolePositions.delete(pane.chat.id);
   if(panes.length)activate(panes[0]);else{active=null;await protectedView();}
   return true;
  }
  if(request.operation==='input'&&typeof request.text==='string'&&(drafts.get(pane.chat.id)||'')===request.text){saveDraft(pane.chat.id,'');if(pane===active&&text.value===request.text)text.value='';}
  if(request.image&&pane.console.image===request.image)files.set(null,pane.console);
  pane.reconnecting=false;if(pane.name.textContent!==pane.chat.name)pane.name.textContent=pane.chat.name;pane.jobsStatus.update(result.jobs);
  pane.console.render(result,request.operation,pane.saved);pane.saved=null;
  if(pane===active&&request.reader===voice.reader)voice.reader.update(result.speech,request.speechGeneration,request.operation==='input'&&typeof request.text==='string');
  pane.ready=true;
  if(pane===active){form.querySelectorAll('textarea,button').forEach(element=>element.disabled=pane.console.preparingImage===true);if(request.operation==='input'&&typeof request.text==='string')voice.afterSend();}
  return true;
 }
 function send(pane,operation,body={}){
  const task=queue.then(async()=>{
   if(closed||pane.closed||operation==='read'&&inputBusy)return false;
   let controller=null;
   try{
    if(pending){const previous=pending;await complete(previous);if(previous.pane===pane&&operation==='read')return true;}
    if(closed||pane.closed)return false;
    if(operation==='open'&&pane.ready)operation='read';
    if(pane.reconnecting&&!document.hidden){try{navigator.vibrate?.(100);}catch{}}
    const currentReader=pane===active?voice.reader:null,speechGeneration=currentReader?.stamp();
    const request=await api('console',{chat_id:pane.chat.id,operation,...(operation==='read'&&pane.console.revision?{screen_revision:pane.console.revision}:{}),...body,...currentReader?.request()});
    pending={...request,pane,operation,text:body.text,image:body.image,reader:currentReader,speechGeneration,deadline:Date.now()+23000};
    if(operation==='read'){
     controller=new AbortController();reading=controller;
     if(inputBusy||document.hidden)controller.abort();
    }
    return await complete(pending,controller?.signal);
   }catch(error){if(controller?.signal.aborted){if(pending?.operation==='read')pending=null;return false;}if(error.status===403){
    try{await api('protected');}catch(authError){if(authError.status===403){voice.stop();show('');location.replace('/auth');return false;}}
   }
   if(error.status===409)pending=null;
   if(!closed&&!pane.closed){pane.jobsStatus.stale();pane.reconnecting=true;pane.name.textContent=pane.chat.name+OaclisSettings.text(operation==='input'?' · Invio non confermato':' · '+error.message);}console.error(error);throw error;}
   finally{if(reading===controller)reading=null;}
  });queue=task.catch(()=>false);return task;
 }
 function schedulePoll(delay=200){clearTimeout(pollTimer);if(!closed&&!document.hidden)pollTimer=setTimeout(poll,delay);}
 async function poll(){
  if(closed||polling)return;polling=true;
  try{if(!document.hidden&&!inputBusy){for(const pane of [active,...panes.filter(pane=>pane!==active)]){if(closed||inputBusy)break;if(pane&&!pane.closed)await send(pane,pane.ready?'read':'open');}}}
  finally{polling=false;}
  schedulePoll();
 }
 async function runInput(task){
  inputBusy=true;clearTimeout(pollTimer);reading?.abort();
  try{return await task();}
  finally{inputBusy=false;schedulePoll();}
 }
 async function submit(pane=active,key='Enter'){
  if(inputBusy||closed||!pane?.ready||pane.console.preparingImage)return false;
  const value=pane===active?text.value:drafts.get(pane.chat.id)||'';saveDraft(pane.chat.id,value);
  return runInput(()=>send(pane,'input',key==='Enter'?{text:value,key,...(pane.console.image?{image:pane.console.image}:{})}:{key}));
 }
 async function stopConsole(pane=active){
  if(inputBusy||closed||!pane?.ready)return false;
  return runInput(()=>send(pane,'stop'));
 }
 const files=view.files=OaclisTerminal.attachImages(view,target=>{if(target){const selected=panes.find(pane=>pane.element.contains(target));if(selected)activate(selected);}return active.console;},chat=>{if(chat.image)imageDrafts.set(chat.id,chat.image);else imageDrafts.delete(chat.id);});
 async function open(selected,beside=false){
  picker.close();
  const existing=panes.find(pane=>pane.chat.id===selected.id);if(existing){activate(existing);return true;}
  const replaced=beside?panes.find(pane=>pane!==active):active,index=replaced?panes.indexOf(replaced):panes.length;
  if(replaced){if(replaced===active){saveDraft(replaced.chat.id,text.value);active=null;}remove(replaced);}
  const pane=add(selected,index);activate(pane);clearTimeout(pollTimer);const ok=await send(pane,'open');schedulePoll();return ok;
 }
 function toggleSplit(){
  if(panes.length===2){remove(panes.find(pane=>pane!==active));layout();bindVoice(active.chat);return;}
  picker.replaceChildren();const heading=document.createElement('h1');heading.textContent=OaclisSettings.text('Affianca chat');const close=document.createElement('button');close.type='button';close.className='voice-close secondary';close.setAttribute('aria-label',OaclisSettings.text('Annulla'));close.innerHTML=OaclisIcon('close');close.onclick=()=>picker.close();picker.append(heading,close);
  for(const [index,selected]of authorizedChats.entries()){const button=chatButton(selected,index,()=>openConsole(selected,true));button.disabled=panes.some(pane=>pane.chat.id===selected.id);picker.append(button);}picker.showModal();
 }
 consoleWorkspace={get active(){return active.console;},open,query:async operation=>{
  if(inputBusy||!active?.ready)throw Error(OaclisSettings.text('Attendi il completamento della richiesta.'));
  return runInput(()=>send(active,operation));
 },create:async(text,icon,model,engine)=>{
  if(inputBusy||!active?.ready)return false;
  return runInput(()=>send(active,'create',{text,icon,model,engine,key:''}));
 },split:()=>panes.length===2,selectPane:position=>activate(panes[['sotto','bottom'].includes(OaclisVoice.normalize(position))?1:0]),list:()=>picker.open?authorizedChats:visibleChatList};
 closeConsole=()=>{if(closed)return;files.destroy();reading?.abort();if(active)saveDraft(active.chat.id,text.value);for(const pane of [...panes])remove(pane);closed=true;clearTimeout(pollTimer);window.visualViewport?.removeEventListener('resize',fitConsole);window.removeEventListener('resize',fitConsole);document.removeEventListener('visibilitychange',resume);document.documentElement.style.removeProperty('--console-height');consoleWorkspace=null;closeConsole=null;};
 text.addEventListener('input',()=>{if(active)saveDraft(active.chat.id,text.value);});
 document.getElementById('back').onclick=()=>protectedView().catch(()=>location.replace('/auth'));
 split.onclick=event=>{event.stopPropagation();toggleSplit();};
 const resume=()=>{if(closed)return;clearTimeout(pollTimer);if(document.hidden)reading?.abort();else{keepConsoleAwake();poll();}};document.addEventListener('visibilitychange',resume);
 const pane=add(chat);activate(pane);keepConsoleAwake();const ready=await send(pane,'open');schedulePoll(ready?200:1000);
}
async function protectedView(){
 const x=await api('protected');
 authorizedChats=x.chats||[];
 show(OaclisSettings.html('<header class="online-brand" aria-label="Oaclis"><img src="/icon-192.png" alt="" width="32" height="32"><span>aclis</span></header><div id="chat-tools"><button id="new-chat" class="secondary" type="button" aria-label="Nuova chat" title="Nuova chat">'+OaclisIcon('plus')+'</button><input id="chat-search" type="search" maxlength="160" aria-label="Cerca chat" placeholder="Cerca chat"></div><div id="authorized-chats" aria-live="polite"></div><button id="lock" class="secondary">Blocca accesso</button><small id="online-version" aria-label="Versione Oaclis"></small>'));
 document.getElementById('new-chat').onclick=newMobileChat;document.getElementById('new-chat').disabled=!authorizedChats.length;
 document.getElementById('chat-search').oninput=()=>renderAuthorizedChats(authorizedChats.filter(chat=>OaclisVoice.normalize(chat.name).includes(OaclisVoice.normalize(document.getElementById('chat-search').value))));
 document.getElementById('online-version').textContent='PWA '+app.dataset.pwaVersion;
 authorizedJobs=x.jobs||[];renderAuthorizedChats(authorizedChats);bindVoice();
 document.getElementById('lock').onclick=e=>run(e.target,async()=>{clearTimeout(timer);voice.stop();drafts.clear();consolePositions.clear();await api('logout');location.replace('/auth');});
 const list=document.getElementById('authorized-chats');
 const refresh=async()=>{if(document.getElementById('authorized-chats')!==list)return;try{const data=await api('protected');if(document.getElementById('authorized-chats')!==list)return;authorizedChats=data.chats||[];authorizedJobs=data.jobs||[];document.getElementById('chat-search').dispatchEvent(new Event('input'));timer=setTimeout(refresh,5000);}catch(e){if(document.getElementById('authorized-chats')!==list)return;if(e.status===403){clearTimeout(timer);voice.stop();show('');location.replace('/auth');}else{message(e.message,true);timer=setTimeout(refresh,5000);}}};
 timer=setTimeout(refresh,5000);
}
async function deviceView(){
 if(!await OaclisMobileSecurity.hasKey(code)){show('');return;}
 const x=await api('status');
 if(x.status==='pending'){show('');timer=setInterval(async()=>{try{const s=await api('status');if(s.status==='approved'){clearInterval(timer);await deviceView();}}catch{clearInterval(timer);app.hidden=true;app.replaceChildren();}},3000);return;}
 const login=async()=>{
  show('');
  try{const data=await api('login-options');const c=await navigator.credentials.get(options(data));await api('login',credential(c));location.replace('/');}
  catch{show('');}
 };
 await login();
}
(async()=>{try{if(invite)await registerView();else if(code){if(authPage)await deviceView();else try{await protectedView();}catch(e){if(e.status===403)location.replace('/auth');else throw e;}}}catch(e){if(invite){show(OaclisSettings.html('<h1>Abbina questo telefono</h1>'));message(e.message,true);}else{voice.stop();show('');}}})();
