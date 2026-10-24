// voice.js v1.33.226
'use strict';
window.OaclisVoice=(()=>{
 const numbers=['uno','due','tre','quattro','cinque','sei','sette','otto','nove','dieci','undici','dodici','tredici','quattordici','quindici','sedici','diciassette','diciotto','diciannove','venti'];
 const normalize=text=>text.normalize('NFKC').trim().toLocaleLowerCase('it-IT').replace(/\s+/g,' ').replace(/[.!?,;:]+$/u,'').trim();
 const number=text=>/^\d+$/.test(text)?Number(text):numbers.indexOf(text)+1;
 const commands=[
  {id:'send',aliases:['invia','invia messaggio'],label:'invia'},
  {id:'back',aliases:['indietro','torna indietro'],label:'indietro'},
  {id:'chatList',aliases:['elenco chat','mostra chat'],label:'elenco chat'},
  {id:'selectChatNumber',pattern:/^seleziona chat numero (.+)$/,label:'seleziona chat numero …'},
  {id:'selectChat',pattern:/^(?:seleziona|apri) chat (.+)$/,label:'apri chat …'},
  {id:'searchChat',pattern:/^cerca chat (.+)$/,label:'cerca chat …'},
  {id:'compose',aliases:['scrivi messaggio'],label:'scrivi messaggio'},
  {id:'newLine',aliases:['a capo','nuova riga'],label:'a capo'},
  {id:'undoDictation',aliases:['annulla dettatura'],label:'annulla dettatura'},
  {id:'clearDraft',aliases:['svuota messaggio'],label:'svuota messaggio'},
  {id:'scrollUp',aliases:['scorri su'],label:'scorri su'},
  {id:'scrollDown',aliases:['scorri giù','scorri in basso'],label:'scorri giù'},
  {id:'help',aliases:['aiuto vocale','mostra comandi'],label:'aiuto vocale'},
  {id:'stop',aliases:['ferma ascolto','disattiva microfono'],label:'ferma ascolto'},
 ];
 function parse(segment){
  const original=segment.trim(),text=normalize(original);
  if(text==='scrivi messaggio')return {type:'command',id:'compose',args:''};
  if(text.startsWith('scrivi '))return {type:'dictation',text:original.replace(/^scrivi\s+/iu,'')};
  const explicit=text.startsWith('comando '),value=explicit?text.slice(8):text;
  for(const item of commands){
   const match=item.pattern?.exec(value);
   if(item.aliases?.includes(value)||match){
    // Keep parameter case/accents as dictated; only the matching copy is normalized.
    const args=match?original.replace(explicit?/^comando\s+/iu:/^/u,'').replace(/^(?:(?:seleziona chat numero)|(?:(?:seleziona|apri|cerca) chat))\s+/iu,'').replace(/[.!?,;:]+$/u,'').trim():'';
    if(args.length>160)return {type:'error',message:'Nome troppo lungo.'};
    return {type:'command',id:item.id,args};
   }
  }
  return explicit?{type:'error',message:'Comando sconosciuto.'}:{type:'dictation',text:segment};
 }
 function resolve(chats,name){
  const target=normalize(name),exact=chats.filter(chat=>normalize(chat.name)===target);
  return exact.length?exact:chats.filter(chat=>normalize(chat.name).includes(target));
 }
 function create(){
  const Recognition=window.SpeechRecognition||window.webkitSpeechRecognition;
  let adapter=null,recognition=null,retiring=null,generation=0,revision=0,target=null,undo=null,pending=null,pendingTimer,retryTimer,retries=0,wanted=false,locked=false,state='idle',disposed=false,utteranceRevision=0;
  let button,status,microphoneState,preview,options,help,root;
  const blocked=()=>locked||adapter?.sending?.();
  const valid=field=>field?.isConnected&&!field.disabled&&!field.readOnly&&adapter?.fields().includes(field)&&['textarea','input'].includes(field.localName)&&field.type!=='password';
  const feedback=text=>{if(status)status.textContent=text;};
  const paint=()=>{if(microphoneState)microphoneState.textContent=({idle:'',starting:'Avvio microfono…',listening:'Ascolto attivo',stopping:'Arresto microfono…',error:'Microfono non attivo'})[state];if(button){button.dataset.state=state;button.setAttribute('aria-pressed',String(wanted));button.title=wanted?'Ferma ascolto':'Attiva voce';button.setAttribute('aria-label',button.title);}};
  const clearPending=()=>{clearTimeout(pendingTimer);pending=null;if(options)options.replaceChildren();};
  const invalidate=()=>{
   clearTimeout(retryTimer);generation++;
   const old=recognition;recognition=null;
   if(old){retiring=old;state='stopping';try{old.abort();}catch{retiring=null;}}
   if(preview)preview.textContent='';if(!retiring)state='idle';paint();
  };
  const stop=()=>{wanted=false;retries=0;clearPending();invalidate();feedback('Ascolto disattivato');};
  const saveUndo=(field,value,start,end)=>{undo={field,value,start,end,after:field.value,revision,chat:adapter?.chatId};};
  function insert(value){
   const field=target;
   if(!valid(field)){feedback('Seleziona un campo oppure pronuncia un comando');return;}
   const before=field.value,start=field.selectionStart??before.length,end=field.selectionEnd??start;
   let addition=value;
   if(value!=='\n'){
    if(start>0&&!/\s$/.test(before.slice(0,start))&&!/^[\s.,!?;:]/.test(addition))addition=' '+addition;
    if(end<before.length&&!/^\s/.test(before.slice(end))&&!/[\s]$/.test(addition))addition+=' ';
   }
   const limit=field.maxLength>=0?field.maxLength:Infinity;
   if(before.length-(end-start)+addition.length>limit){feedback('Limite del campo raggiunto.');return;}
   field.setRangeText(addition,start,end,'end');
   field.dispatchEvent(new Event('input',{bubbles:true}));
   saveUndo(field,before,start,end);feedback('Testo aggiunto');
  }
  function showHelp(){
   help.hidden=!help.hidden;help.replaceChildren();
   if(help.hidden)return;
   const p=document.createElement('p');p.textContent=commands.filter(command=>available(command.id)).map(command=>command.label).join(' · ')+'. Per dettare una parola comando: “scrivi invia”.';help.append(p);
  }
  function available(id){
   if(['stop','help','selectChat','selectChatNumber'].includes(id))return true;
   if(id==='send')return valid(adapter?.compose)&&typeof adapter?.send==='function';
   if(id==='compose')return typeof adapter?.composeFocus==='function';
   if(id==='undoDictation')return !!undo;
   if(id==='newLine')return valid(target)&&target.localName==='textarea';
   if(id==='clearDraft')return !!adapter?.compose?.value;
   return typeof adapter?.[id]==='function';
  }
  function choices(kind,items){
   clearPending();
   pending={kind,items,revision,chat:adapter?.chatId,value:adapter?.compose?.value};
   options.replaceChildren();
   for(const [index,item]of items.entries()){
    const b=document.createElement('button');b.type='button';b.className='secondary';b.textContent=kind==='select'?`${index+1}. ${item.name}`:item.name;
    b.onclick=()=>choose(index);options.append(b);
   }
   const cancel=document.createElement('button');cancel.type='button';cancel.className='secondary';cancel.textContent='Annulla';cancel.onclick=()=>{clearPending();feedback('Operazione annullata');};options.append(cancel);
   pendingTimer=setTimeout(()=>{clearPending();feedback('Scelta scaduta');},kind==='select'?15000:10000);
  }
  async function action(task){
   if(blocked()){feedback('Operazione in corso');return;}
   locked=true;invalidate();clearPending();
   try{const result=await task();feedback(result===false?'Operazione non completata. Riprova.':'Comando eseguito');}catch{feedback('Operazione non completata. Riprova.');}
   finally{locked=false;if(wanted&&!document.hidden&&!disposed)start();}
  }
  function openChat(chat){
   const current=adapter?.chats().find(item=>item.id===chat.id);
   if(!current){feedback('Chat non più disponibile.');return;}
   return action(()=>adapter.selectChat(current));
  }
  function choose(index){
   const choice=pending;
   if(!choice)return;
   if(choice.revision!==revision||choice.chat!==adapter?.chatId){clearPending();feedback('Contesto cambiato.');return;}
   if(!choice.items[index]){feedback('Scegli un numero tra quelli mostrati.');return;}
   if(choice.kind==='select'){const chat=choice.items[index];clearPending();return openChat(chat);}
   if(adapter.compose.value!==choice.value){clearPending();feedback('La bozza è cambiata. Svuotamento annullato.');return;}
   clearPending();target=adapter.compose;target.select();insert('');feedback('Messaggio svuotato');
  }
  async function route(segment){
   if(blocked()){feedback('Operazione in corso');return;}
   if(pending){
    const value=normalize(segment).replace(/^comando /u,'');
    if(['annulla selezione','annulla operazione'].includes(value)){clearPending();feedback('Operazione annullata');return;}
    if(pending.kind==='clear'&&value==='conferma svuotamento'){choose(0);return;}
    const match=/^scegli (.+)$/.exec(value);
    if(pending.kind==='select'&&match){const n=number(match[1]);if(n>=1&&n<=20)choose(n-1);else feedback('Scegli un numero tra quelli mostrati.');return;}
    feedback(pending.kind==='select'?'Scegli uno dei numeri mostrati oppure annulla selezione.':'Pronuncia conferma svuotamento oppure annulla operazione.');return;
   }
   const result=parse(segment);
   if(result.type==='error'){feedback(result.message);return;}
   if(result.type==='dictation'){insert(result.text);return;}
   const {id,args}=result;
   if(!available(id)){feedback('Comando non disponibile in questa schermata.');return;}
   switch(id){
    case 'stop':stop();break;
    case 'help':showHelp();break;
    case 'compose':adapter.composeFocus();target=adapter.compose;feedback('Compositore selezionato');break;
    case 'newLine':insert('\n');break;
    case 'undoDictation':
     if(!valid(undo?.field)||undo.revision!==revision||undo.chat!==adapter?.chatId||undo.field.value!==undo.after){feedback('Annullamento non disponibile dopo altre modifiche.');break;}
     undo.field.value=undo.value;undo.field.setSelectionRange(undo.start,undo.end);undo.field.dispatchEvent(new Event('input',{bubbles:true}));undo=null;feedback('Dettatura annullata');break;
    case 'clearDraft':choices('clear',[{name:'Conferma svuotamento'}]);feedback('Confermare lo svuotamento del messaggio?');break;
    case 'selectChatNumber':{
     const n=number(normalize(args)),chat=adapter.list()[n-1];
     if(n<1||n>20||!chat){feedback('Numero fuori elenco.');break;}await openChat(chat);break;
    }
    case 'selectChat':{
     const matches=resolve(adapter.chats(),args);
     if(!matches.length){feedback('Chat non trovata.');break;}
     if(matches.length===1)await openChat(matches[0]);else{choices('select',matches.slice(0,20));feedback('Più chat corrispondono. Scegli un numero.');}break;
    }
    case 'send':
     if(!adapter.compose?.value.trim()){feedback('Scrivi prima un messaggio.');break;}
     await action(()=>adapter.send());break;
    default:await action(()=>adapter[id](args));
   }
  }
  function retry(){
   if(!wanted||document.hidden||blocked()||disposed)return;
   if(retries>=3){wanted=false;state='error';paint();feedback('Ascolto interrotto. Tocca il microfono per riprovare.');return;}
   const delay=[500,1000,2000][retries++];retryTimer=setTimeout(start,delay);
  }
  function start(){
   if(!Recognition||!window.isSecureContext||disposed||!wanted||document.hidden||blocked()||recognition||retiring)return;
   clearTimeout(retryTimer);const current=++generation,session=new Recognition(),consumed=new Set();recognition=session;
   session.lang='it-IT';session.continuous=true;session.interimResults=true;session.maxAlternatives=1;
   state='starting';paint();utteranceRevision=revision;
   const fresh=()=>!disposed&&recognition===session&&generation===current;
   session.onstart=()=>{if(fresh()){state='listening';paint();}};
   session.onspeechstart=()=>{if(fresh())utteranceRevision=revision;};
   session.onresult=event=>{
    if(!fresh()||blocked()||utteranceRevision!==revision)return;
    const interim=[];
    for(let i=event.resultIndex;i<event.results.length;i++){
     if(!fresh()||blocked())break;
     const result=event.results[i];
     if(result.isFinal){if(consumed.has(i))continue;consumed.add(i);retries=0;preview.textContent='';route(result[0].transcript);}
     else interim.push(result[0].transcript);
    }
    if(fresh())preview.textContent=interim.join(' ');
   };
   session.onerror=event=>{
    if(!fresh())return;
    const terminal=['not-allowed','service-not-allowed','audio-capture','language-not-supported'].includes(event.error);
    const messages={'not-allowed':'Microfono non autorizzato.','service-not-allowed':'Servizio vocale non consentito.','audio-capture':'Microfono non disponibile.','network':'Rete vocale non disponibile.','language-not-supported':'Italiano non disponibile.'};
    feedback(messages[event.error]||'Ascolto interrotto.');
    if(terminal){wanted=false;invalidate();state='error';paint();}
   };
   session.onend=()=>{if(retiring===session){retiring=null;state='idle';paint();if(wanted&&!blocked()&&!disposed)retryTimer=setTimeout(start,0);return;}if(!fresh())return;recognition=null;state='idle';preview.textContent='';paint();retry();};
   try{session.start();}catch{if(fresh()){recognition=null;state='error';paint();feedback('Microfono non avviato.');retry();}}
  }
  function focus(event){
   if(valid(event.target)&&target!==event.target){
    target=event.target;revision++;undo=null;clearPending();
    if(recognition){invalidate();if(wanted&&!blocked())retryTimer=setTimeout(start,0);}
   }
  }
  function input(event){
   if(pending?.kind==='clear'&&event.target===adapter?.compose){clearPending();feedback('La bozza è cambiata. Svuotamento annullato.');}
  }
  function hidden(){if(document.hidden)stop();}
  function bind(next,host){
   root?.remove();button?.remove();invalidate();clearPending();revision++;undo=null;target=null;adapter=next;
   root=document.createElement('section');root.className='voice-panel';root.setAttribute('aria-label','Controlli vocali');
   const line=document.createElement('div');line.className='voice-line';
   status=document.createElement('span');status.setAttribute('aria-live','polite');status.setAttribute('role','status');status.textContent='';
   const helpButton=document.createElement('button');helpButton.type='button';helpButton.className='secondary';helpButton.textContent='?';helpButton.title='Comandi vocali';helpButton.setAttribute('aria-label','Comandi vocali');helpButton.onclick=showHelp;
   microphoneState=document.createElement('span');microphoneState.className='voice-state';microphoneState.setAttribute('aria-live','polite');
   line.append(microphoneState,status,helpButton);preview=document.createElement('div');preview.className='voice-preview';preview.setAttribute('aria-hidden','true');
   options=document.createElement('div');options.className='voice-options';help=document.createElement('div');help.className='voice-help';help.hidden=true;
   root.append(line,preview,options,help);if(host.id==='chat-tools')host.after(root);else document.getElementById('app').append(root);
   button=document.createElement('button');button.type='button';button.className='voice-mic secondary';button.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 10v2a7 7 0 0 0 14 0v-2m-7 9v3m-4 0h8"/></svg>';
   host.append(button);
   button.onclick=()=>{if(wanted)stop();else{wanted=true;retries=0;start();}};
   if(!Recognition||!window.isSecureContext){button.disabled=true;feedback('Voce non disponibile in questo browser.');}
   paint();
   if(wanted&&!blocked())retryTimer=setTimeout(start,0);
  }
  function leave(){invalidate();clearPending();revision++;target=null;adapter=null;undo=null;root=null;button=null;status=null;microphoneState=null;preview=null;options=null;help=null;}
  function ready(){if(adapter?.compose&&valid(adapter.compose)&&!target)target=adapter.compose;}
  function destroy(){stop();disposed=true;leave();document.removeEventListener('focusin',focus);document.removeEventListener('input',input);document.removeEventListener('visibilitychange',hidden);window.removeEventListener('pagehide',stop);}
  document.addEventListener('focusin',focus);document.addEventListener('input',input);document.addEventListener('visibilitychange',hidden);window.addEventListener('pagehide',stop);
  return {bind,leave,ready,stop,destroy,pause:()=>{clearPending();invalidate();},resume:()=>{if(wanted&&!blocked())start();}};
 }
 return {create,parse,resolve,commands,normalize,number};
})();
