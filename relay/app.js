// app.js v1.71.685
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
let authorizedChats=[],visibleChatList=[],consoleActions=null,closeConsole=null,consoleWorkspace=null;
const consolePositions=new Map();
function saveDraft(chatId,value){if(value)drafts.set(chatId,value);else drafts.delete(chatId);}
function bindVoice(chat=null){
 const compose=document.querySelector('#console-input textarea'),search=document.getElementById('chat-search');
 const host=document.getElementById('console-mic')||document.getElementById('console-keys')||document.getElementById('chat-tools');
 if(!host)return;
 voice.bind({chatId:chat?.id,compose,fields:()=>[compose,search].filter(Boolean),chats:()=>authorizedChats,list:()=>consoleWorkspace?.list()||visibleChatList,
  selectChat:async selected=>{if(!authorizedChats.some(item=>item.id===selected.id))return;await openConsole(selected);},
  ...(chat?{splitChat:selected=>openConsole(selected,true),...(consoleWorkspace?.split()?{selectPane:position=>consoleWorkspace.selectPane(position)}:{}),send:async()=>{if(consoleActions?.chatId!==chat.id||!authorizedChats.some(item=>item.id===chat.id))throw Error();return consoleActions.submit();},sending:()=>consoleActions?.sending()===true,commandOnly:()=>consoleActions?.commandOnly()===true,readOn:()=>consoleActions?.readOn(),readOff:()=>consoleActions?.readOff(),readStop:()=>consoleActions?.readStop()}:{}),
 },host);
 voice.ready();
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
function show(html){clearTimeout(timer);closeConsole?.();voice.leave();app.innerHTML=html;app.hidden=false;if(screenWakeLock&&!document.getElementById('console-screen')){screenWakeLock.release().catch(()=>{});screenWakeLock=null;}}
function message(text,error=false){let p=document.getElementById('message');if(!p){p=document.createElement('p');p.id='message';p.setAttribute('role','status');app.append(p);}p.className=error?'error':'status';p.textContent=OaclisSettings.text(text);}
async function api(action,body={}){
 if(action==='console')body={...body,signed:await OaclisMobileSecurity.sign(code,body)};
 const r=await fetch('/auth',{method:'POST',headers:{'Content-Type':'application/json','X-Device-Code':code},body:JSON.stringify({action,...body}),cache:'no-store'});
 if(!r.ok)throw Object.assign(new Error(r.status===403?OaclisSettings.text('Accesso non autorizzato.'):OaclisSettings.text('Collegamento temporaneamente non disponibile. Riprova.')),{status:r.status});
 const result=await r.json();
 OaclisMobileSecurity.update(result.security);
 if(action==='console'&&result.request_id!==JSON.parse(body.signed.payload).id)throw Object.assign(new Error(OaclisSettings.text('Accesso non autorizzato.')),{status:403});
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
function chatIcon(chat){
 const icon=document.createElement('span');icon.className='authorized-icon';icon.setAttribute('aria-hidden','true');
 if(typeof chat.icon==='string'&&/^data:image\/png;base64,iVBORw0KGgo[A-Za-z0-9+/=]*$/.test(chat.icon)&&chat.icon.length<=48000){const image=document.createElement('img');image.src=chat.icon;image.alt='';icon.append(image);}
 else if(Object.hasOwn(OaclisIcons,chat.icon))icon.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">'+OaclisIcons[chat.icon]+'</svg>';
 else icon.textContent=[...chat.name.trim()][0]?.toLocaleUpperCase()||'?';
 if(/^#[a-f0-9]{6}$/i.test(chat.color||''))icon.style.color=chat.color;
 return icon;
}
function renderAuthorizedChats(chats){
 const list=document.getElementById('authorized-chats');
 if(!list)return;
 const signature=JSON.stringify(chats);
 if(list.dataset.signature===signature)return;
 list.dataset.signature=signature;list.replaceChildren();
 visibleChatList=chats.slice();
 if(!chats.length){const p=document.createElement('p');p.textContent=OaclisSettings.text('Nessuna chat autorizzata dal master.');list.append(p);return;}
 const ul=document.createElement('ul');
 for(const [index,chat] of chats.entries()){
  const li=document.createElement('li');li.append(chatButton(chat,index,()=>openConsole(chat)));ul.append(li);
 }
 list.append(ul);
}
function chatButton(chat,index,select){
 const button=document.createElement('button');button.className='secondary authorized-chat';button.setAttribute('aria-label',String(index+1)+'. '+chat.name);
  const number=document.createElement('span');number.className='authorized-number';number.textContent=String(index+1);
  const icon=chatIcon(chat);
  const name=document.createElement('span');name.className='authorized-name';name.textContent=chat.name;
  const play=document.createElement('span');play.className='authorized-play';play.setAttribute('aria-hidden','true');play.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="m9 5 10 7-10 7z"/></svg>';
  button.append(number,icon,name,play);button.onclick=select;return button;
}
function consoleScreen(value){
 const lines=String(value??'').split('\n');
 let last=lines.findLastIndex(line=>line.trim());
 // Only terminal chrome at the screen edges; never remove mentions in messages.
 if(last>=0&&/^\s{2,}(?:GPT-[\w.-]+|gpt-[\w.-]+|o\d[\w.-]*)\b.*(?: · .* · |context left.*(?:shortcuts|agents))/.test(lines[last]))lines.splice(last,1);
 last=lines.findLastIndex(line=>line.trim());
 if(last>=0&&/^\s*[›>]\s*Ask Codex to do anything[.!?]?\s*$/i.test(lines[last]))lines.splice(last,1);
 for(let i=0;i<Math.min(12,lines.length);i++){
  if(!/^\s*[╭┌].*[╮┐]\s*$/.test(lines[i])||!/^\s*[│┃]\s*>_ OpenAI Codex\b/.test(lines[i+1]||''))continue;
  const end=lines.findIndex((line,index)=>index>i&&index<=i+12&&/^\s*[╰└].*[╯┘]\s*$/.test(line));
  if(end>i&&lines.slice(i,end).some(line=>/^\s*[│┃]\s*(?:model|directory):/i.test(line)))lines.splice(i,end-i+1);
  break;
 }
 while(lines.length&&!lines.at(-1).trim())lines.pop();
 while(lines.length&&!lines[0].trim())lines.shift();
 return lines.join('\n');
}
async function openConsole(chat,adjacent=false){
 if(!authorizedChats.some(item=>item.id===chat.id))return false;
 if(consoleWorkspace)return consoleWorkspace.open(chat,adjacent);
 clearTimeout(timer);
 show(OaclisSettings.html('<div id="console-panes"></div><form id="console-input"><textarea aria-label="Messaggio" maxlength="8000" rows="2" placeholder="Scrivi un messaggio" disabled></textarea><button disabled aria-label="Invia messaggio"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 19V5m-6 6 6-6 6 6"/></svg></button></form><div id="console-keys"><button id="back" class="secondary" type="button" aria-label="Chat autorizzate" title="Chat autorizzate">&lt;</button></div><dialog id="console-picker" class="voice-dialog" aria-label="Affianca chat"></dialog>'));
 const panes=[],container=document.getElementById('console-panes'),form=document.getElementById('console-input'),text=form.querySelector('textarea'),keys=document.getElementById('console-keys'),picker=document.getElementById('console-picker');
 let active=null,reader=null,closed=false,pollTimer=null,queue=Promise.resolve(),pending=null,inputBusy=false,polling=false;
 const split=document.createElement('button');split.type='button';split.className='console-split secondary';split.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 12h18"/></svg>';
 const fitConsole=()=>{const following=panes.filter(pane=>pane.follow);document.documentElement.style.setProperty('--console-height',(window.visualViewport?.height??innerHeight)+'px');requestAnimationFrame(()=>{if(!closed)for(const pane of following)if(!pane.closed){pane.follow=true;pane.screen.scrollTop=pane.screen.scrollHeight;}});};
 fitConsole();window.visualViewport?.addEventListener('resize',fitConsole);window.addEventListener('resize',fitConsole);
 function layout(){
  fitConsole();container.dataset.split=String(panes.length===2);panes[0]?.title.append(split);
  split.setAttribute('aria-pressed',String(panes.length===2));split.title=OaclisSettings.text(panes.length===2?'Chat singola':'Affianca chat');split.setAttribute('aria-label',split.title);
 }
 function remove(pane){saveDraft(pane.chat.id,pane===active?text.value:drafts.get(pane.chat.id)||'');consolePositions.set(pane.chat.id,pane.saved||{top:pane.screen.scrollTop,left:pane.screen.scrollLeft,follow:pane.follow});pane.closed=true;pane.element.remove();panes.splice(panes.indexOf(pane),1);}
 function activate(pane){
  if(!pane||pane.closed||closed)return false;
  if(pane===active)return true;
  if(active)saveDraft(active.chat.id,text.value);
  reader?.destroy();active=pane;text.value=drafts.get(pane.chat.id)||'';text.setAttribute('aria-label',OaclisSettings.text('Messaggio')+' · '+pane.chat.name);
  for(const item of panes){const selected=item===pane;item.element.classList.toggle('is-active',selected);item.element.setAttribute('aria-current',String(selected));item.title.id=selected?'console-title':'';item.screen.id=selected?'console-screen':'';}
  reader=OaclisReadAloud.create(document.getElementById('console-read'));
  consoleActions={chatId:pane.chat.id,submit:()=>submit(pane),sending:()=>inputBusy||!pane.ready,commandOnly:()=>reader?.commandOnly()===true,readOn:()=>reader?.enable(),readOff:()=>reader?.disable(),readStop:()=>reader?.skip()};
  bindVoice(pane.chat);form.querySelectorAll('textarea,button').forEach(element=>element.disabled=!pane.ready);voice.ready();layout();return true;
 }
 function add(selected,index=panes.length){
  const element=document.createElement('article');element.className='console-pane';element.setAttribute('aria-label',selected.name);element.innerHTML=OaclisSettings.html('<h1 class="console-title"></h1><pre class="console-screen" tabindex="0" aria-label="Console">Connessione…</pre>');
  const title=element.querySelector('h1'),name=document.createElement('span');name.className='console-name';name.textContent=selected.name;title.append(chatIcon(selected),name);
  const saved=consolePositions.get(selected.id),pane={chat:selected,element,title,name,screen:element.querySelector('pre'),ready:false,closed:false,follow:saved?.follow??true,saved};
  panes.splice(index,0,pane);container.insertBefore(element,container.children[index]||null);
  pane.screen.addEventListener('scroll',()=>{pane.follow=pane.screen.scrollHeight-pane.screen.clientHeight-pane.screen.scrollTop<=24;},{passive:true});
  element.addEventListener('click',event=>{if(!event.target.closest('button'))activate(pane);});element.addEventListener('focusin',event=>{if(!event.target.closest('button'))activate(pane);});
  return pane;
 }
 async function complete(request){
  const deadline=request.deadline;let result;
  do{if(closed)return false;result=await api('console-result',{chat_id:request.pane.chat.id,request_id:request.request_id});}while(result.pending&&Date.now()<deadline);
  if(closed)return false;
  if(result.pending){pending=null;throw Error(OaclisSettings.text('Collegamento temporaneamente non disponibile. Riprova.'));}
  pending=null;if(result.error)throw Error(result.error);
  const pane=request.pane;if(pane.closed)return true;
  if(request.operation==='input'&&typeof request.text==='string'&&(drafts.get(pane.chat.id)||'')===request.text){saveDraft(pane.chat.id,'');if(pane===active&&text.value===request.text)text.value='';}
  pane.reconnecting=false;pane.name.textContent=pane.chat.name;
  if(request.operation==='open'||request.operation==='input')pane.follow=true;
  const top=pane.saved?.top??pane.screen.scrollTop,left=pane.saved?.left??pane.screen.scrollLeft;
  const savedFollow=pane.saved?.follow;if(request.operation==='open'&&savedFollow===false)pane.follow=false;
  OaclisTerminal.render(pane.screen,consoleScreen(result.screen),result.screen_ansi);pane.saved=null;
  if(pane===active&&request.reader===reader){if(request.operation==='input'&&typeof request.text==='string')reader.defer(2000);reader.update(result.speech,request.speechGeneration);}
  if(pane.follow)pane.screen.scrollTop=pane.screen.scrollHeight;else{pane.screen.scrollTop=top;pane.screen.scrollLeft=left;}
  pane.ready=true;
  if(pane===active){form.querySelectorAll('textarea,button').forEach(element=>element.disabled=false);voice.ready();voice.resume();if(request.operation==='input'&&typeof request.text==='string')voice.afterSend();}
  return true;
 }
 function send(pane,operation,body={}){
  const task=queue.then(async()=>{
   if(closed||pane.closed)return false;
   try{
    if(pending){const previous=pending;await complete(previous);if(previous.pane===pane&&operation==='read')return true;}
    if(closed||pane.closed)return false;
    if(operation==='open'&&pane.ready)operation='read';
    if(pane.reconnecting&&!document.hidden){try{navigator.vibrate?.(100);}catch{}}
    const currentReader=pane===active?reader:null,speechGeneration=currentReader?.stamp();
    const request=await api('console',{chat_id:pane.chat.id,operation,...body,...currentReader?.request()});
    pending={...request,pane,operation,text:body.text,reader:currentReader,speechGeneration,deadline:Date.now()+23000};
    return await complete(pending);
   }catch(error){if(error.status===403){voice.stop();show('');location.replace('/auth');}else if(!closed&&!pane.closed){pane.reconnecting=true;pane.name.textContent=pane.chat.name+OaclisSettings.text(operation==='input'?' · Invio non confermato':' · Riconnessione…');}return false;}
  });queue=task.catch(()=>false);return task;
 }
 async function poll(){
  if(closed||polling)return;polling=true;
  try{if(!document.hidden&&!inputBusy){for(const pane of [active,...panes.filter(pane=>pane!==active)]){if(closed||inputBusy)break;if(pane&&!pane.closed)await send(pane,pane.ready?'read':'open');}}}
  finally{polling=false;if(!closed){clearTimeout(pollTimer);pollTimer=setTimeout(poll,200);}}
 }
 async function submit(pane=active,key='Enter'){
  if(inputBusy||closed||!pane?.ready)return false;
  inputBusy=true;clearTimeout(pollTimer);
  const value=pane===active?text.value:drafts.get(pane.chat.id)||'';saveDraft(pane.chat.id,value);
  try{return await send(pane,'input',key==='Enter'?{text:value,key}:{key});}
  finally{inputBusy=false;if(!closed){voice.resume();clearTimeout(pollTimer);pollTimer=setTimeout(poll,200);}}
 }
 async function open(selected,beside=false){
  if(!authorizedChats.some(chat=>chat.id===selected.id))return false;
  picker.close();
  const existing=panes.find(pane=>pane.chat.id===selected.id);if(existing){activate(existing);return true;}
  const replaced=beside?panes.find(pane=>pane!==active):active,index=replaced?panes.indexOf(replaced):panes.length;
  if(replaced){if(replaced===active){saveDraft(replaced.chat.id,text.value);reader?.destroy();reader=null;active=null;}remove(replaced);}
  const pane=add(selected,index);activate(pane);clearTimeout(pollTimer);const ok=await send(pane,'open');if(!closed){clearTimeout(pollTimer);pollTimer=setTimeout(poll,200);}return ok;
 }
 function toggleSplit(){
  if(panes.length===2){remove(panes.find(pane=>pane!==active));layout();bindVoice(active.chat);return;}
  picker.replaceChildren();const heading=document.createElement('h1');heading.textContent=OaclisSettings.text('Affianca chat');const close=document.createElement('button');close.type='button';close.className='voice-close secondary';close.setAttribute('aria-label',OaclisSettings.text('Annulla'));close.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg>';close.onclick=()=>picker.close();picker.append(heading,close);
  for(const [index,selected]of authorizedChats.entries()){const button=chatButton(selected,index,()=>openConsole(selected,true));button.disabled=panes.some(pane=>pane.chat.id===selected.id);picker.append(button);}picker.showModal();
 }
 consoleWorkspace={open,split:()=>panes.length===2,selectPane:position=>activate(panes[['sotto','bottom'].includes(OaclisVoice.normalize(position))?1:0]),list:()=>picker.open?authorizedChats:visibleChatList};
 closeConsole=()=>{if(closed)return;if(active)saveDraft(active.chat.id,text.value);reader?.destroy();for(const pane of [...panes])remove(pane);closed=true;clearTimeout(pollTimer);window.visualViewport?.removeEventListener('resize',fitConsole);window.removeEventListener('resize',fitConsole);document.removeEventListener('visibilitychange',resume);document.documentElement.style.removeProperty('--console-height');consoleActions=null;consoleWorkspace=null;closeConsole=null;};
 text.addEventListener('input',()=>{if(active)saveDraft(active.chat.id,text.value);});form.onsubmit=async event=>{event.preventDefault();await submit();};text.onkeydown=event=>{if(event.key==='Enter'&&!event.shiftKey&&!event.isComposing){event.preventDefault();form.requestSubmit();}};
 document.getElementById('back').onclick=()=>protectedView().catch(()=>location.replace('/auth'));
 for(const [key,label]of [['Escape','Esc'],['1','1'],['2','2'],['3','3'],['C-c','Ctrl+C']]){const button=document.createElement('button');button.type='button';button.textContent=label;button.dataset.key=key;button.onclick=()=>submit(active,key);keys.append(button);}
 for(const id of ['console-mic','console-read']){const slot=document.createElement('span');slot.id=id;keys.append(slot);}
 const info=document.createElement('button');info.type='button';info.className='secondary';info.title=OaclisSettings.text('Comandi vocali');info.setAttribute('aria-label',info.title);info.innerHTML='<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-10v2"/></svg>';info.onclick=()=>voice.showHelp();keys.append(info);
 split.onclick=event=>{event.stopPropagation();toggleSplit();};
 const resume=()=>{if(document.visibilityState==='visible'&&!closed){keepConsoleAwake();clearTimeout(pollTimer);poll();}};document.addEventListener('visibilitychange',resume);
 const pane=add(chat);activate(pane);keepConsoleAwake();const ready=await send(pane,'open');if(!closed)pollTimer=setTimeout(poll,ready?200:1000);
}
async function protectedView(){
 voice.pause();
 let x;try{x=await api('protected');}catch(error){voice.resume();throw error;}
 authorizedChats=x.chats||[];
 show(OaclisSettings.html('<header class="online-brand" aria-label="Oaclis"><img src="/icon-192.png" alt="" width="32" height="32"><span>aclis</span></header><div id="chat-tools"><input id="chat-search" type="search" maxlength="160" aria-label="Cerca chat" placeholder="Cerca chat"></div><div id="authorized-chats" aria-live="polite"></div><button id="lock" class="secondary">Blocca accesso</button><small id="online-version" aria-label="Versione Oaclis"></small>'));
 document.getElementById('chat-search').oninput=()=>renderAuthorizedChats(authorizedChats.filter(chat=>OaclisVoice.normalize(chat.name).includes(OaclisVoice.normalize(document.getElementById('chat-search').value))));
 document.getElementById('online-version').textContent='PWA '+app.dataset.pwaVersion;
 renderAuthorizedChats(authorizedChats);bindVoice();
 document.getElementById('lock').onclick=e=>run(e.target,async()=>{clearTimeout(timer);voice.stop();drafts.clear();consolePositions.clear();await api('logout');location.replace('/auth');});
 const list=document.getElementById('authorized-chats');
 const refresh=async()=>{if(document.getElementById('authorized-chats')!==list)return;try{const data=await api('protected');if(document.getElementById('authorized-chats')!==list)return;authorizedChats=data.chats||[];document.getElementById('chat-search').dispatchEvent(new Event('input'));timer=setTimeout(refresh,5000);}catch(e){if(document.getElementById('authorized-chats')!==list)return;if(e.status===403){clearTimeout(timer);voice.stop();show('');location.replace('/auth');}else{message(e.message,true);timer=setTimeout(refresh,5000);}}};
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
