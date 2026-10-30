// voice.js v1.38.558
'use strict';
window.OaclisVoice=(()=>{
 const locale=OaclisSettings,grammar=locale.voice;
 const preferences={startMic:false,startVoice:false,autoSend:true,waitFive:true,pauseAfterSend:false};
 try{const saved=JSON.parse(localStorage.getItem('oaclis-voice-preferences')||'{}');for(const key of Object.keys(preferences))if(typeof saved?.[key]==='boolean')preferences[key]=saved[key];}catch{}
 const numbers=grammar.numbers;
 const normalize=text=>text.normalize('NFKC').trim().toLocaleLowerCase(locale.speechLanguage).replace(/\s+/g,' ').replace(/[.!?,;:]+$/u,'').trim();
 const number=text=>/^\d+$/.test(text)?Number(text):numbers.indexOf(text)+1;
 const commands=grammar.commands.map(([id,phrases,label])=>Array.isArray(phrases)?{id,aliases:phrases,label:phrases[0]}:{id,pattern:phrases,label});
 function parse(segment){
  const original=segment.trim(),text=normalize(original);
  if(text.startsWith(grammar.literal+' '))return {type:'dictation',text:original.replace(new RegExp('^'+grammar.literal+'\\s+','iu'),'')};
  const explicit=text.startsWith(grammar.explicit+' '),value=explicit?text.slice(grammar.explicit.length+1):text;
  const raw=explicit?original.replace(new RegExp('^'+grammar.explicit+'\\s+','iu'),''):original;
  for(const item of commands){
   const match=item.pattern?.exec(value);
   if(item.aliases?.includes(value)||match){
    const args=match?(item.pattern.exec(raw)?.[1]||match[1]).replace(/[.!?,;:]+$/u,'').trim():'';
    if(args.length>160)return {type:'error',message:locale.text('Nome troppo lungo.')};
    return {type:'command',id:item.id,args};
   }
  }
  return explicit?{type:'error',message:locale.text('Comando sconosciuto.')}:{type:'dictation',text:segment};
 }
 function resolve(chats,name){
  const target=normalize(name),exact=chats.filter(chat=>normalize(chat.name)===target);
  return exact.length?exact:chats.filter(chat=>normalize(chat.name).includes(target));
 }
 function create(){
  const Recognition=window.SpeechRecognition||window.webkitSpeechRecognition;
  // Android continuous mode promotes cumulative partial results to final results.
  const singleUtterance=navigator.userAgentData?.platform==='Android'||/Android/i.test(navigator.userAgent);
  let adapter=null,recognition=null,recorder=null,retiring=null,generation=0,revision=0,target=null,pending=null,pendingTimer,retryTimer,retries=0,wanted=preferences.startMic,paused=false,locked=false,state='idle',disposed=false,utteranceRevision=0;
  let button,status,preview,options,help,root,autoSendTimer;
  const cancelAutoSend=()=>{clearTimeout(autoSendTimer);autoSendTimer=null;};
  function scheduleAutoSend(){
   cancelAutoSend();
   if(!preferences.autoSend)return;
   const field=adapter?.compose,value=field?.value,chat=adapter?.chatId;
   if(target!==field||!value?.trim()||typeof adapter?.send!=='function')return;
   autoSendTimer=setTimeout(()=>{
    autoSendTimer=null;
    if(preferences.autoSend&&wanted&&!paused&&!document.hidden&&!blocked()&&!pending&&valid(field)&&adapter?.compose===field&&adapter?.chatId===chat&&field.value===value)action(()=>adapter.send());
   },preferences.waitFive?5000:3000);
  }
  const blocked=()=>locked||adapter?.sending?.();
  const valid=field=>field?.isConnected&&!field.disabled&&!field.readOnly&&adapter?.fields().includes(field)&&['textarea','input'].includes(field.localName)&&field.type!=='password';
  const feedback=text=>{if(status)status.textContent=locale.text(text);};
  const paint=()=>{
   if(!button)return;
   const mode=!wanted?'off':paused?'paused':state==='listening'&&!blocked()?'listening':'busy';
   button.dataset.state=state;button.dataset.mode=mode;button.setAttribute('aria-pressed',String(wanted));
   button.title=locale.text(({off:'Attiva voce',paused:'Dettatura in pausa · start rec per riprendere',listening:'Dettatura attiva · clic per spegnere',busy:'Collegamento o elaborazione vocale · clic per spegnere'})[mode]);
   button.setAttribute('aria-label',button.title);
  };
  const clearPending=()=>{clearTimeout(pendingTimer);pending=null;if(options){if(options.open)options.close();options.replaceChildren();}};
  function releaseSession(session){
   for(const name of ['onstart','onspeechstart','onspeechend','onresult','onerror','onend'])session[name]=null;
  }
  const invalidate=()=>{
   clearTimeout(retryTimer);retryTimer=null;generation++;
   const old=recognition;recognition=null;
   if(old){retiring=old;state='stopping';try{old.abort();}catch{releaseSession(old);recorder=null;retiring=null;}}
   if(preview)preview.textContent='';if(!retiring)state='idle';paint();
  };
  const stop=()=>{cancelAutoSend();if(wanted)window.OaclisReadAloud?.feedback('off');wanted=false;paused=false;retries=0;clearPending();invalidate();feedback('Ascolto disattivato');};
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
   feedback('Testo aggiunto');return true;
  }
  function showHelp(){
   if(help.open)return;
   help.replaceChildren();
   const close=document.createElement('button');close.type='button';close.className='voice-close secondary';close.setAttribute('aria-label',locale.text('Chiudi comandi vocali'));
   close.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg>';close.onclick=()=>help.close();
   const title=document.createElement('h1');title.textContent=locale.text('Comandi vocali');
   const header=document.createElement('header');header.className='voice-help-header';header.append(title,close);
   const settings=document.createElement('details');settings.className='voice-settings';settings.setAttribute('aria-label',locale.text('Impostazioni'));
   const summary=document.createElement('summary');summary.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">'+(window.OaclisIcons?.settings||'')+'</svg>';
   summary.append(document.createTextNode(locale.text('Impostazioni')));
   const chevron=document.createElementNS('http://www.w3.org/2000/svg','svg');chevron.setAttribute('viewBox','0 0 24 24');chevron.setAttribute('aria-hidden','true');chevron.innerHTML='<path d="m9 5 7 7-7 7"/>';summary.append(chevron);settings.append(summary);
   for(const [key,label]of [['startMic',"Microfono acceso all'avvio"],['startVoice',"Voce accesa all'avvio"],['autoSend','Invio automatico'],['waitFive','Attesa di 5 secondi (altrimenti 3)'],['pauseAfterSend','Pausa dettatura dopo invio']]){
    const row=document.createElement('label'),check=document.createElement('input');check.type='checkbox';check.checked=preferences[key];
    check.onchange=()=>{preferences[key]=check.checked;try{localStorage.setItem('oaclis-voice-preferences',JSON.stringify(preferences));}catch{}if(!preferences.autoSend)cancelAutoSend();else if(key==='waitFive'&&autoSendTimer)scheduleAutoSend();};
    row.append(check,document.createTextNode(locale.text(label)));settings.append(row);
   }
   const list=document.createElement('ul');list.className='voice-command-list';
   for(const command of commands.filter(command=>available(command.id))){const item=document.createElement('li'),name=document.createElement('strong'),detail=document.createElement('span');name.textContent=command.aliases?.join(' / ')||command.label;detail.textContent=grammar.descriptions[command.id]||'';item.append(name,detail);list.append(item);}
   help.append(header,settings,list);help.showModal();
  }
  function available(id){
   if(['stop','help','pauseRec','resumeRec','selectChat'].includes(id))return true;
   if(id==='send')return valid(adapter?.compose)&&typeof adapter?.send==='function';
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
   const cancel=document.createElement('button');cancel.type='button';cancel.className='secondary';cancel.textContent=locale.text('Annulla');cancel.onclick=()=>{clearPending();feedback('Operazione annullata');};options.append(cancel);options.showModal();
   pendingTimer=setTimeout(()=>{clearPending();feedback('Scelta scaduta');},kind==='select'?15000:10000);
  }
  async function action(task){
   if(blocked()){feedback('Operazione in corso');return;}
   cancelAutoSend();locked=true;invalidate();clearPending();
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
  async function route(segment,commandsOnly=false){
   if(blocked()){feedback('Operazione in corso');return;}
   const result=parse(segment);
   const audio=result.type==='command'&&['readOn','readOff','readStop'].includes(result.id);
   if((commandsOnly||adapter?.commandOnly?.())&&!audio)return;
   if(audio){if(available(result.id))await action(()=>adapter[result.id]());else feedback('Comando non disponibile in questa schermata.');return;}
   if(result.type==='command'){
    cancelAutoSend();
    if(result.id==='stop'){stop();return;}
    if(result.id==='pauseRec'){if(!paused)window.OaclisReadAloud?.feedback('pause');paused=true;clearPending();preview.textContent='';paint();return;}
    if(result.id==='resumeRec'){if(paused)window.OaclisReadAloud?.feedback('on');paused=false;paint();return;}
    if(result.id==='help'){if(help.open)help.close();else showHelp();return;}
   }
   if(paused)return;
   if(pending){
    const value=normalize(segment).replace(new RegExp('^'+grammar.explicit+' '),'');
    if(grammar.cancel.includes(value)){clearPending();feedback('Operazione annullata');return;}
    if(pending.kind==='clear'&&value===grammar.confirm){choose(0);return;}
    const match=grammar.choose.exec(value);
    if(pending.kind==='select'&&match){const n=number(match[1]);if(n>=1&&n<=20)choose(n-1);else feedback('Scegli un numero tra quelli mostrati.');return;}
    feedback(pending.kind==='select'?'Scegli uno dei numeri mostrati oppure annulla selezione.':'Pronuncia conferma svuotamento oppure annulla operazione.');return;
   }
   if(result.type==='error'){feedback(result.message);return;}
   if(result.type==='dictation'){if(insert(result.text))scheduleAutoSend();return;}
   const {id,args}=result;
   if(!available(id)){feedback('Comando non disponibile in questa schermata.');return;}
   switch(id){
    case 'clearDraft':clearPending();adapter.compose.value='';adapter.compose.dispatchEvent(new Event('input',{bubbles:true}));feedback('Messaggio svuotato');break;
    case 'selectChat':{
     const n=number(normalize(args));
     if(n>0){const chat=adapter.list()[n-1];if(!chat){feedback('Numero fuori elenco.');break;}await openChat(chat);break;}
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
   const delay=[500,1000,2000][retries++];retryTimer=setTimeout(()=>{retryTimer=null;start();},delay);
  }
  function start(){
   if(!Recognition||!window.isSecureContext||disposed||!wanted||document.hidden||blocked()||recognition||retiring)return;
   clearTimeout(retryTimer);retryTimer=null;const current=++generation,session=(recorder??=new Recognition());let consumed=-1,sessionError=null,commandsOnly=false;recognition=session;
   session.lang=locale.speechLanguage;session.continuous=!singleUtterance;session.interimResults=false;session.maxAlternatives=1;
   state='starting';paint();utteranceRevision=revision;
   const fresh=()=>!disposed&&recognition===session&&generation===current;
   session.onstart=()=>{if(fresh()){state='listening';paint();}};
   session.onspeechstart=()=>{if(fresh()){cancelAutoSend();commandsOnly=adapter?.commandOnly?.()===true;utteranceRevision=revision;state='listening';paint();}};
   session.onspeechend=()=>{if(fresh()){state='processing';paint();}};
   session.onresult=event=>{
    if(!fresh()||blocked()||utteranceRevision!==revision)return;
    state='processing';paint();
    const interim=[];
    for(let i=event.resultIndex;i<event.results.length;i++){
     if(!fresh()||blocked())break;
     const result=event.results[i];
     if(result.isFinal){if(i<=consumed)continue;consumed=i;retries=0;preview.textContent='';route(result[0].transcript,commandsOnly);}
     else {cancelAutoSend();interim.push(result[0].transcript);}
    }
    if(fresh()){preview.textContent=paused||commandsOnly||adapter?.commandOnly?.()?'':interim.join(' ');state=interim.length?'processing':'listening';paint();}
   };
   session.onerror=event=>{
    if(!fresh())return;
    sessionError=event.error;
    if(singleUtterance&&event.error==='no-speech'){feedback('In attesa della voce');return;}
    const terminal=['not-allowed','service-not-allowed','audio-capture','language-not-supported'].includes(event.error);
    const messages={'not-allowed':'Microfono non autorizzato.','service-not-allowed':'Servizio vocale non consentito.','audio-capture':'Microfono non disponibile.','network':'Rete vocale non disponibile.','language-not-supported':'Italiano non disponibile.'};
    feedback(messages[event.error]||'Ascolto interrotto.');
    if(terminal){wanted=false;invalidate();state='error';paint();}
   };
   session.onend=()=>{
    if(retiring===session){releaseSession(session);retiring=null;state='idle';paint();clearTimeout(retryTimer);retryTimer=null;if(wanted&&!document.hidden&&!blocked()&&!disposed)retryTimer=setTimeout(()=>{retryTimer=null;start();},0);return;}
    if(!fresh())return;
    releaseSession(session);recognition=null;state='idle';preview.textContent='';paint();
    const normalEnd=singleUtterance&&(!sessionError||sessionError==='no-speech');
    if(normalEnd&&wanted&&!document.hidden&&!blocked())retryTimer=setTimeout(()=>{retryTimer=null;start();},consumed>=0?200:750);
    else retry();
   };
   try{session.start();}catch{if(fresh()){releaseSession(session);recognition=null;state='error';paint();feedback('Microfono non avviato.');retry();}}
  }
  function focus(event){
   if(valid(event.target)&&target!==event.target){
    cancelAutoSend();target=event.target;revision++;clearPending();
    if(recognition){invalidate();if(wanted&&!blocked())retryTimer=setTimeout(()=>{retryTimer=null;start();},0);}
   }
  }
  function input(event){
   if(event.target===adapter?.compose)cancelAutoSend();
   if(pending?.kind==='clear'&&event.target===adapter?.compose){clearPending();feedback('La bozza è cambiata. Svuotamento annullato.');}
  }
  function hidden(){if(document.hidden)stop();}
  function bind(next,host){
   cancelAutoSend();
   root?.remove();button?.remove();invalidate();clearPending();revision++;target=null;adapter=next;
   root=document.createElement('section');root.className='voice-panel';root.setAttribute('aria-label',locale.text('Controlli vocali'));
   status=document.createElement('span');status.className='voice-sr';status.setAttribute('aria-live','polite');status.setAttribute('role','status');
   preview=document.createElement('span');preview.className='voice-preview voice-sr';preview.setAttribute('aria-hidden','true');
   options=document.createElement('dialog');options.className='voice-options voice-dialog';options.setAttribute('aria-label',locale.text('Conferma comando vocale'));options.addEventListener('cancel',event=>{event.preventDefault();clearPending();});
   help=document.createElement('dialog');help.className='voice-help voice-dialog';help.setAttribute('aria-label',locale.text('Comandi vocali'));
   root.append(status,preview,options,help);document.getElementById('app').append(root);
   button=document.createElement('button');button.type='button';button.className='voice-mic secondary';button.innerHTML='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 10v2a7 7 0 0 0 14 0v-2m-7 9v3m-4 0h8"/></svg>';
   host.append(button);
   button.onclick=()=>{if(wanted)stop();else{window.OaclisReadAloud?.feedback('on');wanted=true;paused=false;retries=0;start();}};
   if(!Recognition||!window.isSecureContext){button.disabled=true;feedback('Voce non disponibile in questo browser.');}
   paint();
   if(wanted&&!blocked())retryTimer=setTimeout(()=>{retryTimer=null;start();},0);
  }
  function leave(){cancelAutoSend();invalidate();clearPending();revision++;target=null;adapter=null;root=null;button=null;status=null;preview=null;options=null;help=null;}
  function ready(){if(adapter?.compose&&valid(adapter.compose)&&!target)target=adapter.compose;}
  function afterSend(){
   if(preferences.pauseAfterSend&&wanted&&!paused){cancelAutoSend();paused=true;window.OaclisReadAloud?.feedback('pause');paint();}
  }
  function destroy(){stop();disposed=true;leave();recorder=null;document.removeEventListener('focusin',focus);document.removeEventListener('input',input);document.removeEventListener('visibilitychange',hidden);window.removeEventListener('pagehide',stop);}
  document.addEventListener('focusin',focus);document.addEventListener('input',input);document.addEventListener('visibilitychange',hidden);window.addEventListener('pagehide',stop);
  return {bind,leave,ready,stop,destroy,showHelp,afterSend,pause:()=>{cancelAutoSend();clearPending();invalidate();},resume:()=>{if(wanted&&!blocked()&&!retryTimer)start();}};
 }
 return {create,parse,resolve,commands,normalize,number,preferences};
})();
