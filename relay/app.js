// app.js v1.21.141
'use strict';
if('serviceWorker' in navigator)navigator.serviceWorker.register('/oaclis/sw.js',{scope:'/oaclis/'}).catch(()=>{});
const app=document.getElementById('app');
window.addEventListener('hashchange',()=>{if(location.hash)location.reload();});
const fragment=new URLSearchParams(location.hash.slice(1));
const authPage=location.pathname.replace(/\/$/,'')==='/oaclis/auth';
if(location.hash&&!authPage)location.replace('/oaclis/auth'+location.hash);
let admin=fragment.get('admin')||sessionStorage.getItem('oaclis-test-admin')||'';
let invite=fragment.get('pair')||sessionStorage.getItem('oaclis-test-invite')||'';
let code=localStorage.getItem('oaclis-test-code')||'';
history.replaceState(null,'',location.pathname);
if(admin)sessionStorage.setItem('oaclis-test-admin',admin);
if(fragment.get('pair')&&fragment.get('pair')!==sessionStorage.getItem('oaclis-test-invite')){code='';localStorage.removeItem('oaclis-test-code');}
if(invite)sessionStorage.setItem('oaclis-test-invite',invite);
let timer,screenWakeLock=null,wakePending=false;
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
function show(html){app.innerHTML=html;app.hidden=false;if(screenWakeLock&&!document.getElementById('console-screen')){screenWakeLock.release().catch(()=>{});screenWakeLock=null;}}
function message(text,error=false){let p=document.getElementById('message');if(!p){p=document.createElement('p');p.id='message';p.setAttribute('role','status');app.append(p);}p.className=error?'error':'status';p.textContent=text;}
async function api(action,body={}){
 const r=await fetch('/oaclis/auth',{method:'POST',headers:{'Content-Type':'application/json','X-Admin-Key':admin,'X-Device-Code':code},body:JSON.stringify({action,...body}),cache:'no-store'});
 if(!r.ok){if(r.status===403)throw new Error('Accesso non autorizzato.');throw new Error('Verifica non riuscita. Riprova.');}
 return r.json();
}
function buffer(s){return Uint8Array.from(atob(s.replace(/-/g,'+').replace(/_/g,'/')),c=>c.charCodeAt(0));}
function encode(b){return btoa(String.fromCharCode(...new Uint8Array(b))).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');}
function options(data){const p=data.publicKey;p.challenge=buffer(p.challenge);if(p.user)p.user.id=buffer(p.user.id);for(const c of p.allowCredentials||[])c.id=buffer(c.id);for(const c of p.excludeCredentials||[])c.id=buffer(c.id);return data;}
function credential(c){const r=c.response;const out={id:encode(c.rawId),clientDataJSON:encode(r.clientDataJSON)};for(const k of ['attestationObject','authenticatorData','signature','userHandle'])if(r[k])out[k]=encode(r[k]);return out;}
async function run(button,fn){button.disabled=true;try{await fn();}catch(e){message(e.name==='NotAllowedError'?'Operazione annullata o scaduta. Puoi riprovare.':e.message,true);}finally{button.disabled=false;}}
async function adminList(){
 const {devices}=await api('admin-list');const list=document.getElementById('devices');list.replaceChildren();
 if(!devices.length){const p=document.createElement('p');p.textContent='Nessun telefono abbinato.';list.append(p);}
 for(const d of devices){const row=document.createElement('section');row.className='row';const h=document.createElement('h2');h.textContent=d.name;const p=document.createElement('p');p.textContent=({pending:'In attesa di approvazione',approved:'Autorizzato',revoked:'Revocato'})[d.status];row.append(h,p);if(d.status!=='revoked'){const b=document.createElement('button');b.textContent=d.status==='pending'?'Autorizza':'Revoca';b.className=d.status==='pending'?'':'danger';b.onclick=()=>run(b,async()=>{if(d.status==='approved'&&!confirm('Revocare l’accesso a questo telefono? La prova sarà bloccata immediatamente.'))return;await api(d.status==='pending'?'approve':'revoke',{id:d.id});await adminList();});row.append(b);}list.append(row);}
}
async function adminView(){
 await api('admin-list');show('<h1>Autorizza il telefono</h1><p>Prova separata da Oaclis. Crea un invito e aprilo nel browser del telefono; poi approva la richiesta qui.</p><button id="invite">Crea invito · valido 10 minuti</button><div id="invitation"></div><div id="devices"></div>');
 document.getElementById('invite').onclick=e=>run(e.target,async()=>{const x=await api('invite');const area=document.getElementById('invitation');area.replaceChildren();const p=document.createElement('p');p.textContent='Invia questo collegamento solo al telefono da abbinare:';const a=document.createElement('a');a.href=x.url;a.textContent=x.url;a.className='invite';const b=document.createElement('button');b.textContent='Copia invito';b.className='secondary';b.onclick=()=>run(b,async()=>{await navigator.clipboard.writeText(x.url);message('Invito copiato.');});area.append(p,a,b);});
 await adminList();timer=setInterval(()=>adminList().catch(()=>message('Collegamento interrotto. Ricarica per riprovare.',true)),4000);
}
async function registerView(){
 if(!window.PublicKeyCredential||!window.isSecureContext)throw new Error('Apri questa pagina in un browser aggiornato con HTTPS.');
 if(!code){code=Array.from(crypto.getRandomValues(new Uint8Array(32)),x=>x.toString(16).padStart(2,'0')).join('');localStorage.setItem('oaclis-test-code',code);}
 show('<h1>Abbina questo telefono</h1><p>Creerai una passkey e un codice casuale per questa installazione. Poi la richiesta dovrà essere approvata dal PC.</p><label for="name">Nome del telefono</label><input id="name" maxlength="60" value="Il mio telefono" autocomplete="off"><button id="register">Crea passkey e richiedi accesso</button>');
 document.getElementById('register').onclick=e=>run(e.target,async()=>{const data=await api('register-options',{invite,name:document.getElementById('name').value});const c=await navigator.credentials.create(options(data));await api('register',credential(c));invite='';sessionStorage.removeItem('oaclis-test-invite');await deviceView();});
}
function renderAuthorizedChats(chats){
 const list=document.getElementById('authorized-chats');
 if(!list)return;
 const signature=JSON.stringify(chats);
 if(list.dataset.signature===signature)return;
 list.dataset.signature=signature;list.replaceChildren();
 if(!chats.length){const p=document.createElement('p');p.textContent='Nessuna chat autorizzata dal master.';list.append(p);return;}
 const ul=document.createElement('ul');
 for(const chat of chats){const li=document.createElement('li');const button=document.createElement('button');button.className='secondary';button.textContent=chat.name;button.onclick=()=>openConsole(chat);li.append(button);ul.append(li);}
 list.append(ul);
}
async function openConsole(chat){
 clearTimeout(timer);
 show('<h1 id="console-title"></h1><pre id="console-screen" tabindex="0" aria-label="Console">Connessione…</pre><form id="console-input"><textarea aria-label="Messaggio" maxlength="8000" rows="2" placeholder="Scrivi un messaggio" disabled></textarea><button disabled aria-label="Invia messaggio"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 19V5m-6 6 6-6 6 6"/></svg></button></form><div id="console-keys"><button id="back" class="secondary" type="button" aria-label="Chat autorizzate" title="Chat autorizzate">&lt;</button></div>');
 document.getElementById('console-title').textContent=chat.name;
 keepConsoleAwake();
 const fitConsole=()=>document.documentElement.style.setProperty('--console-height',(window.visualViewport?.height??innerHeight)+'px');
 fitConsole();window.visualViewport?.addEventListener('resize',fitConsole);window.addEventListener('resize',fitConsole);
 const screen=document.getElementById('console-screen'),form=document.getElementById('console-input'),text=form.querySelector('textarea');
 let busy=false,closed=false;
 document.getElementById('back').onclick=()=>{closed=true;window.visualViewport?.removeEventListener('resize',fitConsole);window.removeEventListener('resize',fitConsole);document.documentElement.style.removeProperty('--console-height');document.removeEventListener('visibilitychange',resume);clearTimeout(timer);protectedView().catch(()=>location.replace('/oaclis/auth'));};
 const send=async(operation,body={})=>{
  if(busy||closed)return false;busy=true;
  try{
   const request=await api('console',{chat_id:chat.id,operation,...body});
   const deadline=Date.now()+23000;let result;
   do{await new Promise(resolve=>setTimeout(resolve,700));if(closed)return false;result=await api('console-result',{chat_id:chat.id,request_id:request.request_id});}while(result.pending&&Date.now()<deadline);
   if(result.pending)throw new Error('Il master non risponde. Riapri la console.');
   if(result.error)throw new Error(result.error);
   screen.textContent=result.screen;screen.scrollTop=screen.scrollHeight;
   form.querySelectorAll('textarea,button').forEach(element=>element.disabled=false);return true;
  }catch(e){if(!closed)screen.textContent=e.message;return false;}finally{busy=false;}
 };
 const poll=async()=>{if(closed)return;if(document.hidden||busy){timer=setTimeout(poll,1500);return;}if(await send('read')){clearTimeout(timer);timer=setTimeout(poll,1500);}};
 const resume=()=>{if(document.visibilityState==='visible'&&!closed){clearTimeout(timer);poll();}};
 document.addEventListener('visibilitychange',resume);
 form.onsubmit=async event=>{event.preventDefault();if(busy)return;clearTimeout(timer);if(await send('input',{text:text.value,key:'Enter'}))text.value='';if(!closed)timer=setTimeout(poll,1500);};
 text.onkeydown=event=>{if(event.key==='Enter'&&!event.shiftKey&&!event.isComposing){event.preventDefault();form.requestSubmit();}};
 for(const [key,label] of [['Enter','↵'],['Escape','Esc'],['Tab','Tab'],['Up','↑'],['Down','↓'],['C-c','Ctrl+C']]){
  const button=document.createElement('button');button.type='button';button.textContent=label;button.onclick=async()=>{if(busy)return;clearTimeout(timer);await send('input',{key});if(!closed)timer=setTimeout(poll,1500);};document.getElementById('console-keys').append(button);
 }
 if(await send('open')){clearTimeout(timer);timer=setTimeout(poll,1500);}
}
async function protectedView(){
 const x=await api('protected');
 show('<h1>Chat autorizzate</h1><div id="authorized-chats" aria-live="polite"></div><button id="lock" class="secondary">Blocca accesso</button>');
 renderAuthorizedChats(x.chats||[]);
 document.getElementById('lock').onclick=e=>run(e.target,async()=>{clearTimeout(timer);await api('logout');location.replace('/oaclis/auth');});
 const refresh=async()=>{try{const data=await api('protected');if(!document.getElementById('authorized-chats'))return;renderAuthorizedChats(data.chats||[]);timer=setTimeout(refresh,5000);}catch{clearTimeout(timer);app.hidden=true;app.replaceChildren();}};
 timer=setTimeout(refresh,5000);
}
async function deviceView(){
 const x=await api('status');
 if(x.status==='pending'){show('<h1>Richiesta inviata</h1><p>La passkey è pronta. Approva questo telefono dalla pagina sul PC.</p>');timer=setInterval(async()=>{try{const s=await api('status');if(s.status==='approved'){clearInterval(timer);await deviceView();}}catch{clearInterval(timer);app.hidden=true;app.replaceChildren();}},3000);return;}
 show('<h1>Accedi a Oaclis</h1><p>Conferma con la passkey del telefono per aprire l’area riservata.</p><button id="login">Accedi con passkey</button>');document.getElementById('login').onclick=e=>run(e.target,async()=>{const data=await api('login-options');const c=await navigator.credentials.get(options(data));await api('login',credential(c));location.replace('/oaclis/');});
}
(async()=>{try{if(admin)await adminView();else if(invite)await registerView();else if(code){if(authPage)await deviceView();else try{await protectedView();}catch{location.replace('/oaclis/auth');}}}catch(e){if(admin||invite){show('<h1>Prova accesso</h1>');message(e.message,true);}else{app.hidden=true;app.replaceChildren();}}})();
