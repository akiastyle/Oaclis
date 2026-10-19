// app.js v1.13.47
'use strict';
const app=document.getElementById('app');
window.addEventListener('hashchange',()=>{if(location.hash)location.reload();});
const fragment=new URLSearchParams(location.hash.slice(1));
let admin=fragment.get('admin')||sessionStorage.getItem('oaclis-test-admin')||'';
let invite=fragment.get('pair')||sessionStorage.getItem('oaclis-test-invite')||'';
let code=localStorage.getItem('oaclis-test-code')||'';
history.replaceState(null,'',location.pathname);
if(admin)sessionStorage.setItem('oaclis-test-admin',admin);
if(fragment.get('pair')&&fragment.get('pair')!==sessionStorage.getItem('oaclis-test-invite')){code='';localStorage.removeItem('oaclis-test-code');}
if(invite)sessionStorage.setItem('oaclis-test-invite',invite);
let timer;
function show(html){app.innerHTML=html;app.hidden=false;}
function message(text,error=false){let p=document.getElementById('message');if(!p){p=document.createElement('p');p.id='message';p.setAttribute('role','status');app.append(p);}p.className=error?'error':'status';p.textContent=text;}
async function api(action,body={}){
 const r=await fetch('./index.php',{method:'POST',headers:{'Content-Type':'application/json','X-Admin-Key':admin,'X-Device-Code':code},body:JSON.stringify({action,...body}),cache:'no-store'});
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
async function protectedView(){const x=await api('protected');show('<h1>Accesso autorizzato</h1><p id="identity"></p><p>Codice dell’installazione e passkey verificati dal server. Nessun dato Oaclis è collegato a questa prova.</p><button id="lock" class="secondary">Blocca accesso</button>');document.getElementById('identity').textContent=x.name;document.getElementById('lock').onclick=e=>run(e.target,async()=>{clearInterval(timer);await api('logout');await deviceView();});timer=setInterval(()=>api('protected').catch(()=>{clearInterval(timer);app.hidden=true;app.replaceChildren();}),5000);}
async function deviceView(){
 const x=await api('status');
 if(x.status==='pending'){show('<h1>Richiesta inviata</h1><p>La passkey è pronta. Approva questo telefono dalla pagina sul PC.</p>');timer=setInterval(async()=>{try{const s=await api('status');if(s.status==='approved'){clearInterval(timer);await deviceView();}}catch{clearInterval(timer);app.hidden=true;app.replaceChildren();}},3000);return;}
 show('<h1>Accedi alla prova</h1><p>Conferma con la passkey del telefono per aprire l’area riservata.</p><button id="login">Accedi con passkey</button>');document.getElementById('login').onclick=e=>run(e.target,async()=>{const data=await api('login-options');const c=await navigator.credentials.get(options(data));await api('login',credential(c));await protectedView();});
}
(async()=>{try{if(admin)await adminView();else if(invite)await registerView();else if(code)await deviceView();}catch(e){if(admin||invite){show('<h1>Prova accesso</h1>');message(e.message,true);}else{app.hidden=true;app.replaceChildren();}}})();
