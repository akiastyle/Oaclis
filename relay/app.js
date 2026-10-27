// app.js v1.42.416
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
let authorizedChats=[],visibleChatList=[],consoleActions=null,closeConsole=null;
function saveDraft(chatId,value){if(value)drafts.set(chatId,value);else drafts.delete(chatId);}
function bindVoice(chat=null){
 const compose=document.querySelector('#console-input textarea'),search=document.getElementById('chat-search');
 const host=document.getElementById('console-keys')||document.getElementById('chat-tools');
 if(!host)return;
 voice.bind({chatId:chat?.id,compose,fields:()=>[compose,search].filter(Boolean),chats:()=>authorizedChats,list:()=>visibleChatList,
  selectChat:async selected=>{if(!authorizedChats.some(item=>item.id===selected.id))return;await openConsole(selected);},
  ...(chat?{send:async()=>{if(consoleActions?.chatId!==chat.id||!authorizedChats.some(item=>item.id===chat.id))throw Error();return consoleActions.submit();},back:()=>protectedView(),chatList:()=>protectedView(),sending:()=>consoleActions?.sending()===true}:{}),
  ...(!chat?{searchChat:query=>{search.value=query;search.dispatchEvent(new Event('input',{bubbles:true}));}}:{}),
  scrollUp:()=>{const area=document.getElementById('console-screen')||document.getElementById('authorized-chats');area.scrollBy({top:-area.clientHeight*.75,behavior:'smooth'});},
  scrollDown:()=>{const area=document.getElementById('console-screen')||document.getElementById('authorized-chats');area.scrollBy({top:area.clientHeight*.75,behavior:'smooth'});},
  ...(chat?{composeFocus:()=>compose.focus()}:{}),
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
function show(html){closeConsole?.();voice.leave();app.innerHTML=html;app.hidden=false;if(screenWakeLock&&!document.getElementById('console-screen')){screenWakeLock.release().catch(()=>{});screenWakeLock=null;}}
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
  const li=document.createElement('li'),button=document.createElement('button');button.className='secondary authorized-chat';button.setAttribute('aria-label',String(index+1)+'. '+chat.name);
  const number=document.createElement('span');number.className='authorized-number';number.textContent=String(index+1);
  const icon=document.createElement('span');icon.className='authorized-icon';icon.setAttribute('aria-hidden','true');
  if(typeof chat.icon==='string'&&/^data:image\/png;base64,iVBORw0KGgo[A-Za-z0-9+/=]*$/.test(chat.icon)&&chat.icon.length<=48000){const image=document.createElement('img');image.src=chat.icon;image.alt='';icon.append(image);}
  else if(Object.hasOwn(OaclisIcons,chat.icon))icon.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">'+OaclisIcons[chat.icon]+'</svg>';
  else icon.textContent=[...chat.name.trim()][0]?.toLocaleUpperCase()||'?';
  if(/^#[a-f0-9]{6}$/i.test(chat.color||''))icon.style.color=chat.color;
  const name=document.createElement('span');name.className='authorized-name';name.textContent=chat.name;
  const play=document.createElement('span');play.className='authorized-play';play.setAttribute('aria-hidden','true');play.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="m9 5 10 7-10 7z"/></svg>';
  button.append(number,icon,name,play);button.onclick=()=>openConsole(chat);li.append(button);ul.append(li);
 }
 list.append(ul);
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
async function openConsole(chat){
 clearTimeout(timer);
 show(OaclisSettings.html('<h1 id="console-title"></h1><pre id="console-screen" tabindex="0" aria-label="Console">Connessione…</pre><form id="console-input"><textarea aria-label="Messaggio" maxlength="8000" rows="2" placeholder="Scrivi un messaggio" disabled></textarea><button disabled aria-label="Invia messaggio"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 19V5m-6 6 6-6 6 6"/></svg></button></form><div id="console-keys"><button id="back" class="secondary" type="button" aria-label="Chat autorizzate" title="Chat autorizzate">&lt;</button></div>'));
 const title=document.getElementById('console-title');title.textContent=chat.name;
 keepConsoleAwake();
 const screen=document.getElementById('console-screen'),form=document.getElementById('console-input'),text=form.querySelector('textarea');
 text.value=drafts.get(chat.id)||'';
 text.addEventListener('input',()=>saveDraft(chat.id,text.value));
 let busy=false,closed=false,idle=Promise.resolve(),pending=null,followOutput=true,inputBusy=false,consoleReady=false;
 screen.addEventListener('scroll',()=>{followOutput=screen.scrollHeight-screen.clientHeight-screen.scrollTop<=24;},{passive:true});
 const fitConsole=()=>{
  const stick=followOutput||screen.scrollHeight-screen.clientHeight-screen.scrollTop<=24;
  document.documentElement.style.setProperty('--console-height',(window.visualViewport?.height??innerHeight)+'px');
  if(stick)requestAnimationFrame(()=>{if(!closed){followOutput=true;screen.scrollTop=screen.scrollHeight;}});
 };
 fitConsole();window.visualViewport?.addEventListener('resize',fitConsole);window.addEventListener('resize',fitConsole);
 closeConsole=()=>{if(closed)return;saveDraft(chat.id,text.value);closed=true;window.visualViewport?.removeEventListener('resize',fitConsole);window.removeEventListener('resize',fitConsole);document.documentElement.style.removeProperty('--console-height');document.removeEventListener('visibilitychange',resume);clearTimeout(timer);consoleActions=null;closeConsole=null;};
 document.getElementById('back').onclick=()=>protectedView().catch(()=>location.replace('/auth'));
 const send=async(operation,body={})=>{
  if(busy&&operation==='input')await idle;
  if(busy||closed)return false;busy=true;
  let done;idle=new Promise(resolve=>{done=resolve;});
  try{
   const request=operation==='read'&&pending?pending:Object.assign(await api('console',{chat_id:chat.id,operation,...body}),{operation,text:body.text});
   pending=request;
   const deadline=Date.now()+23000;let result;
   do{if(closed)return false;result=await api('console-result',{chat_id:chat.id,request_id:request.request_id});}while(result.pending&&Date.now()<deadline);
   if(result.pending){pending=null;throw new Error(OaclisSettings.text('Collegamento temporaneamente non disponibile. Riprova.'));}
   const completed=pending;pending=null;
   if(result.error)throw new Error(result.error);
   if(completed.operation==='input'&&text.value===completed.text){text.value='';saveDraft(chat.id,'');}
   title.textContent=chat.name;
   if(completed.operation==='open'||completed.operation==='input')followOutput=true;
   const output=consoleScreen(result.screen);
   const top=screen.scrollTop,left=screen.scrollLeft;
   OaclisTerminal.render(screen,output,result.screen_ansi);
   if(followOutput)screen.scrollTop=screen.scrollHeight;
   else {
    if(screen.scrollTop!==top)screen.scrollTop=top;
    if(screen.scrollLeft!==left)screen.scrollLeft=left;
   }
   consoleReady=true;form.querySelectorAll('textarea,button').forEach(element=>element.disabled=false);voice.ready();voice.resume();return true;
  }catch(e){if(e.status===403){voice.stop();show('');closed=true;location.replace('/auth');}else if(!closed)title.textContent=chat.name+(operation==='input'?OaclisSettings.text(' · Invio non confermato'):OaclisSettings.text(' · Riconnessione…'));return false;}finally{busy=false;done();}
 };
 const poll=async()=>{if(closed)return;if(document.hidden||busy){timer=setTimeout(poll,1500);return;}const ok=await send('read');if(!closed){clearTimeout(timer);timer=setTimeout(poll,ok?200:1000);}};
 const resume=()=>{if(document.visibilityState==='visible'&&!closed){clearTimeout(timer);poll();}};
 document.addEventListener('visibilitychange',resume);
 const submit=async()=>{
  if(inputBusy||closed)return false;
  inputBusy=true;voice.pause();clearTimeout(timer);
  const value=text.value;
  try{return await send('input',{text:value,key:'Enter'});}finally{inputBusy=false;if(!closed){voice.resume();timer=setTimeout(poll,200);}}
 };
 consoleActions={chatId:chat.id,submit,sending:()=>inputBusy||!consoleReady};
 form.onsubmit=async event=>{event.preventDefault();await submit();};
 text.onkeydown=event=>{if(event.key==='Enter'&&!event.shiftKey&&!event.isComposing){event.preventDefault();form.requestSubmit();}};
 for(const [key,label] of [['Enter','↵'],['Escape','Esc'],['Tab','Tab'],['Up','↑'],['Down','↓'],['C-c','Ctrl+C']]){
  const button=document.createElement('button');button.type='button';button.textContent=label;button.onclick=async()=>{clearTimeout(timer);await send('input',{key});if(!closed)timer=setTimeout(poll,200);};document.getElementById('console-keys').append(button);
 }
 bindVoice(chat);
 const ready=await send('open');if(!closed){clearTimeout(timer);timer=setTimeout(poll,ready?200:1000);}
}
async function protectedView(){
 voice.pause();
 let x;try{x=await api('protected');}catch(error){voice.resume();throw error;}
 authorizedChats=x.chats||[];
 show(OaclisSettings.html('<header class="online-brand" aria-label="Oaclis"><img src="/icon-192.png" alt="" width="32" height="32"><span>aclis</span></header><div id="chat-tools"><input id="chat-search" type="search" maxlength="160" aria-label="Cerca chat" placeholder="Cerca chat"></div><div id="authorized-chats" aria-live="polite"></div><button id="lock" class="secondary">Blocca accesso</button><small id="online-version" aria-label="Versione Oaclis"></small>'));
 document.getElementById('chat-search').oninput=()=>renderAuthorizedChats(authorizedChats.filter(chat=>OaclisVoice.normalize(chat.name).includes(OaclisVoice.normalize(document.getElementById('chat-search').value))));
 document.getElementById('online-version').textContent=x.version||'';
 renderAuthorizedChats(authorizedChats);bindVoice();
 document.getElementById('lock').onclick=e=>run(e.target,async()=>{clearTimeout(timer);voice.stop();drafts.clear();await api('logout');location.replace('/auth');});
 const refresh=async()=>{try{const data=await api('protected');if(!document.getElementById('authorized-chats'))return;authorizedChats=data.chats||[];document.getElementById('online-version').textContent=data.version||'';document.getElementById('chat-search').dispatchEvent(new Event('input'));timer=setTimeout(refresh,5000);}catch(e){if(e.status===403){clearTimeout(timer);voice.stop();show('');location.replace('/auth');}else{message(e.message,true);timer=setTimeout(refresh,5000);}}};
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
