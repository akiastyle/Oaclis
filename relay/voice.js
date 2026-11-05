// voice.js v2.52.1090
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
  let session=null,restartTimer=null,retries=0;
  let adapter=null,revision=0,target=null,pending=null,pendingTimer,wanted=false,initialized=false,paused=false,state='idle',disposed=false,utteranceRevision=0,recognitionIssue=null;
  let button,status,preview,options,help,root,autoSendTimer,reader=null,controls=null,info=null;
  const events=[];
  function trace(event,detail={}){events.push({time:new Date().toISOString(),event,...detail});if(events.length>100)events.shift();}
  const cancelAutoSend=()=>{clearTimeout(autoSendTimer);autoSendTimer=null;};
  function scheduleAutoSend(){
   cancelAutoSend();
   if(!preferences.autoSend)return;
   const field=adapter?.compose,value=field?.value,chat=adapter?.chatId;
   if(target!==field||!value?.trim()||!adapter.view)return;
   autoSendTimer=setTimeout(()=>{
    autoSendTimer=null;
    if(preferences.autoSend&&wanted&&!paused&&!document.hidden&&!pending&&valid(field)&&adapter?.compose===field&&adapter?.chatId===chat&&field.value===value)action(()=>adapter.view.command('send'));
   },preferences.waitFive?5000:3000);
  }
  const valid=field=>field?.isConnected&&!field.disabled&&!field.readOnly&&adapter?.fields().includes(field)&&['textarea','input'].includes(field.localName)&&field.type!=='password';
  const feedback=text=>{trace('feedback',{text});if(status)status.textContent=locale.text(text);};
  const paint=()=>{
   if(!button)return;
   if(button.disabled){button.dataset.mode='off';button.setAttribute('aria-pressed','false');button.title=locale.text('Voce non disponibile in questo browser.');button.setAttribute('aria-label',button.title);return;}
   const mode=!wanted?'off':paused?'paused':state==='listening'?'listening':'busy';
   button.dataset.state=state;button.dataset.mode=mode;button.setAttribute('aria-pressed',String(wanted));
   button.title=locale.text(({off:'Attiva voce',paused:'Dettatura in pausa · riprendi',listening:'Dettatura attiva · clic per spegnere',busy:'Collegamento o elaborazione vocale · clic per spegnere'})[mode]);
   button.setAttribute('aria-label',button.title);
  };
  const clearPending=()=>{clearTimeout(pendingTimer);pending=null;if(options){if(options.open)options.close();options.replaceChildren();}};
  function scheduleRecognition(hadResult){
   if(!wanted||disposed)return;
   if(!hadResult)retries++;
   if(retries>8){state='error';feedback('Microfono non avviato.');paint();return;}
   const delay=Math.min(400*2**Math.min(retries,4),6000);
   state='starting';trace('recognition:scheduled',{delay,retries});paint();
   restartTimer=setTimeout(()=>{restartTimer=null;if(wanted&&!disposed&&!session)startRecognition('end');},delay);
  }
  function startRecognition(reason){
   const current=session=new Recognition();let consumed=-1,finalTranscript='',hadResult=false;
   recognitionIssue=null;utteranceRevision=revision;state='starting';
   current.lang=locale.speechLanguage;current.continuous=true;current.interimResults=true;current.maxAlternatives=1;
   trace('recognition:config',{continuous:current.continuous,interimResults:current.interimResults,language:current.lang});
   const fresh=()=>wanted&&!disposed&&session===current;
   current.onstart=()=>{trace('recognition:start');if(fresh()){state='listening';paint();}};
   current.onspeechstart=()=>{trace('recognition:speechstart');if(fresh()){cancelAutoSend();utteranceRevision=revision;state='listening';paint();}};
   current.onspeechend=()=>{trace('recognition:speechend');if(fresh()){state='processing';paint();}};
   for(const name of ['audiostart','audioend','soundstart','soundend'])current['on'+name]=()=>trace('recognition:'+name);
   current.onresult=event=>{
    if(!fresh()||!adapter||utteranceRevision!==revision){trace('recognition:result-ignored',{wanted,revision,utteranceRevision});return;}
    hadResult=true;retries=0;state='processing';paint();
    const interim=[];
    for(let i=event.resultIndex;i<event.results.length;i++){
     if(!fresh())break;
     const result=event.results[i];
     if(result.isFinal){
      trace('recognition:final',{index:i,text:result[0].transcript.slice(0,200)});
      if(i<=consumed)continue;consumed=i;preview.textContent='';
      let transcript=result[0].transcript;
      const dictation=parse(transcript).type==='dictation';
      if(dictation&&finalTranscript){
       const previous=normalize(finalTranscript).split(' '),words=transcript.trim().split(/\s+/);
       for(let overlap=Math.min(previous.length,words.length);overlap>0;overlap--){
        if(normalize(words.slice(0,overlap).join(' '))===previous.slice(-overlap).join(' ')){transcript=words.slice(overlap).join(' ');break;}
       }
      }
      finalTranscript=dictation?result[0].transcript:'';
      if(transcript)route(transcript);
     }
     else {cancelAutoSend();interim.push(result[0].transcript);}
    }
    if(fresh()){preview.textContent=paused?'':interim.join(' ');state=interim.length?'processing':'listening';paint();}
   };
   current.onerror=event=>{
    trace('recognition:error',{error:event.error,message:event.message,wanted});
    if(!fresh())return;
    recognitionIssue={event:'error',error:event.error,message:event.message||''};
    const messages={'no-speech':'In attesa della voce','not-allowed':'Microfono non autorizzato.','service-not-allowed':'Servizio vocale non consentito.','audio-capture':'Microfono non disponibile.','network':'Rete vocale non disponibile.','language-not-supported':'Italiano non disponibile.'};
    state='error';feedback(locale.text(messages[event.error]||'Ascolto interrotto.')+' ['+event.error+']');paint();
   };
   current.onend=()=>{
    trace('recognition:end',{wanted,state,paused,chat:adapter?.chatId});
    if(session!==current)return;
    session=null;
    if(!wanted||disposed){state='idle';paint();return;}
    if(['not-allowed','service-not-allowed','audio-capture','language-not-supported'].includes(recognitionIssue?.error)){state='error';paint();return;}
    scheduleRecognition(hadResult);
   };
   try{trace('call:start',{reason});current.start();}
   catch(error){session=null;recognitionIssue={event:'exception',error:error.name,message:error.message};trace('recognition:exception',recognitionIssue);scheduleRecognition(false);}
  }
  function setListening(on,silent=true){
   if(on===wanted||disposed||!Recognition||!window.isSecureContext)return;
   wanted=on;paused=false;clearTimeout(restartTimer);restartTimer=null;cancelAutoSend();clearPending();
   if(!silent)OaclisReadAloud.feedback();
   if(preview)preview.textContent='';
   try{
    if(on){retries=0;startRecognition('user');}
    else{state='idle';const previous=session;session=null;if(previous){trace('call:abort',{caller:new Error().stack.split('\n').slice(1,5).join(' | ')});previous.abort();}}
   }catch(error){trace('recognition:exception',{name:error.name,message:error.message});state='error';feedback('Microfono non avviato.');paint();throw error;}
   paint();
  }
  const stop=(silent=true)=>setListening(false,silent);
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
   close.innerHTML=OaclisIcon('close');close.onclick=()=>help.close();
   const title=document.createElement('h1');title.textContent=locale.text('Comandi vocali');
   const header=document.createElement('header');header.className='voice-help-header';header.append(title,close);
   const settings=document.createElement('details');settings.className='voice-settings';settings.setAttribute('aria-label',locale.text('Impostazioni'));
   const summary=document.createElement('summary');summary.innerHTML=OaclisIcon('settings');
   summary.append(document.createTextNode(locale.text('Impostazioni')));
   const chevron=document.createElementNS('http://www.w3.org/2000/svg','svg');chevron.setAttribute('viewBox','0 0 24 24');chevron.setAttribute('aria-hidden','true');chevron.innerHTML=OaclisIcons.chevronRight;summary.append(chevron);settings.append(summary);
   for(const [key,label]of [['startMic',"Microfono acceso all'avvio"],['startVoice',"Voce accesa all'avvio"],['autoSend','Invio automatico'],['waitFive','Attesa di 5 secondi (altrimenti 3)'],['pauseAfterSend','Pausa dettatura dopo invio']]){
    const row=document.createElement('label'),check=document.createElement('input');check.type='checkbox';check.checked=preferences[key];
    check.onchange=()=>{preferences[key]=check.checked;try{localStorage.setItem('oaclis-voice-preferences',JSON.stringify(preferences));}catch{}if(!preferences.autoSend)cancelAutoSend();else if(key==='waitFive'&&autoSendTimer)scheduleAutoSend();};
    row.append(check,document.createTextNode(locale.text(label)));settings.append(row);
   }
   const list=document.createElement('ul');list.className='voice-command-list';
   for(const command of commands.filter(command=>command.id==='clearDraft'||available(command.id))){const item=document.createElement('li'),name=document.createElement('strong'),detail=document.createElement('span');name.textContent=command.aliases?.join(' / ')||command.label;detail.textContent=grammar.descriptions[command.id]||'';item.append(name,detail);list.append(item);}
   help.append(header);
   if(recognitionIssue){const error=document.createElement('p');error.setAttribute('role','alert');error.textContent=JSON.stringify(recognitionIssue);help.append(error);}
   const diagnostic=document.createElement('details');diagnostic.className='voice-trace';
   const diagnosticTitle=document.createElement('summary');diagnosticTitle.textContent=locale.text('Diagnostica vocale');
   const log=document.createElement('pre');log.textContent=events.map(event=>JSON.stringify(event)).join('\n');
   const copy=document.createElement('button');copy.type='button';copy.className='secondary';copy.textContent=locale.text('Copia log');copy.onclick=()=>navigator.clipboard.writeText(log.textContent);
   diagnostic.append(diagnosticTitle,log,copy);help.append(diagnostic,settings,list);help.showModal();
  }
  function available(id){
   if(['stop','help','pauseRec','resumeRec','selectChat'].includes(id))return true;
   if(['readOn','readOff','readStop'].includes(id))return !!adapter.chatId;
   if(['send','clearDraft','takePhoto','pickImage','removeImage'].includes(id))return !!adapter.view;
   return typeof adapter?.[id]==='function';
  }
  function choices(items,method='selectChat'){
   clearPending();
   pending={items,method,revision,chat:adapter?.chatId};
   options.replaceChildren();
   for(const [index,item]of items.entries()){
    const b=document.createElement('button');b.type='button';b.className='secondary';b.textContent=`${index+1}. ${item.name}`;
    b.onclick=()=>choose(index);options.append(b);
   }
   const cancel=document.createElement('button');cancel.type='button';cancel.className='secondary';cancel.textContent=locale.text('Annulla');cancel.onclick=()=>{clearPending();feedback('Operazione annullata');};options.append(cancel);options.showModal();
   pendingTimer=setTimeout(()=>{clearPending();feedback('Scelta scaduta');},15000);
  }
  async function action(task){
   cancelAutoSend();clearPending();
   const result=await task();feedback(result===false?'Operazione non completata. Riprova.':'Comando eseguito');
  }
  function openChat(chat,method='selectChat'){
   const current=adapter?.chats().find(item=>item.id===chat.id);
   if(!current){feedback('Chat non più disponibile.');return;}
   return action(()=>adapter[method](current));
  }
  function choose(index){
   const choice=pending;
   if(!choice)return;
   if(choice.revision!==revision||choice.chat!==adapter?.chatId){clearPending();feedback('Contesto cambiato.');return;}
   if(!choice.items[index]){feedback('Scegli un numero tra quelli mostrati.');return;}
   const chat=choice.items[index];clearPending();return openChat(chat,choice.method);
  }
  async function route(segment){
   const result=parse(segment);
   trace('route:'+result.type,{command:result.id,args:result.args});
   const audio=result.type==='command'&&['readOn','readOff','readStop','selectPane'].includes(result.id);
   const media=result.type==='command'&&['takePhoto','pickImage','removeImage'].includes(result.id);
   if(result.type==='command'){
    cancelAutoSend();
    if(result.id==='stop'){stop(false);return;}
    if(result.id==='pauseRec'){if(!paused)OaclisReadAloud.feedback();paused=true;clearPending();preview.textContent='';paint();return;}
    if(result.id==='resumeRec'){if(paused)OaclisReadAloud.feedback();paused=false;paint();return;}
    if(result.id==='help'){if(help.open)help.close();else showHelp();return;}
   }
   if(audio){if(available(result.id)){if(['readOn','readOff','readStop'].includes(result.id))reader[{readOn:'enable',readOff:'disable',readStop:'skip'}[result.id]]();else adapter[result.id](result.args);}return;}
   if(media){await action(()=>adapter.view.command(result.id));return;}
   if(paused)return;
   if(pending){
    const value=normalize(segment).replace(new RegExp('^'+grammar.explicit+' '),'');
    if(grammar.cancel.includes(value)){clearPending();feedback('Operazione annullata');return;}
    const match=grammar.choose.exec(value);
    if(match){const n=number(match[1]);if(n>=1&&n<=20)choose(n-1);else feedback('Scegli un numero tra quelli mostrati.');return;}
    feedback('Scegli uno dei numeri mostrati oppure annulla selezione.');return;
   }
   if(result.type==='error'){feedback(result.message);return;}
   if(result.type==='dictation'){if(insert(result.text))scheduleAutoSend();return;}
   const {id,args}=result;
   if(!available(id)){feedback('Comando non disponibile in questa schermata.');return;}
   switch(id){
    case 'clearDraft':clearPending();adapter.view.command('clearDraft');feedback('Messaggio svuotato');break;
    case 'selectChat':case 'splitChat':{
     const n=number(normalize(args));
     if(n>0){const chat=adapter.list()[n-1];if(!chat){feedback('Numero fuori elenco.');break;}await openChat(chat,id);break;}
     const matches=resolve(adapter.chats(),args);
     if(!matches.length){feedback('Chat non trovata.');break;}
     if(matches.length===1)await openChat(matches[0],id);else{choices(matches.slice(0,20),id);feedback('Più chat corrispondono. Scegli un numero.');}break;
    }
    case 'send':
     await action(()=>adapter.view.command('send'));break;
    default:await action(()=>adapter[id](args));
   }
  }
  function focus(event){
   if(valid(event.target)&&target!==event.target){
    cancelAutoSend();target=event.target;revision++;clearPending();
   }
  }
  function input(event){
   if(event.target===adapter?.compose)cancelAutoSend();
  }
  function hidden(){trace('page:visibility',{state:document.visibilityState});if(document.hidden){cancelAutoSend();clearPending();}}
  function bind(next,host){
   const changed=!adapter||adapter.chatId!==next.chatId||adapter.compose!==next.compose;
   if(changed){trace('target',{chat:next.chatId});cancelAutoSend();clearPending();revision++;target=next.compose||next.fields()[0];}
   adapter=next;
   if(!button){
    controls=host;controls.classList.add('voice-controls');
    root=document.createElement('section');root.className='voice-panel';root.setAttribute('aria-label',locale.text('Controlli vocali'));
    status=document.createElement('span');status.className='voice-sr';status.setAttribute('aria-live','polite');status.setAttribute('role','status');
    preview=document.createElement('span');preview.className='voice-preview voice-sr';preview.setAttribute('aria-hidden','true');
    options=document.createElement('dialog');options.className='voice-options voice-dialog';options.setAttribute('aria-label',locale.text('Conferma comando vocale'));options.addEventListener('cancel',event=>{event.preventDefault();clearPending();});
    help=document.createElement('dialog');help.className='voice-help voice-dialog';help.setAttribute('aria-label',locale.text('Comandi vocali'));
    root.append(status,preview,options,help);
    button=document.createElement('button');button.type='button';button.className='voice-mic secondary icon-button';button.dataset.voiceAction='microphone';button.innerHTML=OaclisIcon('microphone');controls.append(button);
    button.onclick=()=>setListening(!wanted,false);
    if(!Recognition||!window.isSecureContext){button.disabled=true;feedback('Voce non disponibile in questo browser.');}
    info=document.createElement('button');info.type='button';info.className='voice-info secondary icon-button';info.dataset.voiceAction='commands';info.title=locale.text('Comandi vocali');info.setAttribute('aria-label',info.title);info.innerHTML=OaclisIcon('info');info.onclick=showHelp;controls.append(info);
   }else if(host!==controls){host.replaceWith(controls);}
   if(next.chatId&&!reader){
    reader=OaclisReadAloud.create(controls,trace);const read=controls.querySelector('.read-aloud');read.dataset.voiceAction='read';read.classList.add('icon-button');controls.insertBefore(read,info);
   }else if(changed)reader?.reset(!!next.chatId);
   const read=controls.querySelector('.read-aloud');if(read)read.hidden=!next.chatId;
   (document.getElementById('app')||document.body).append(root);paint();
   if(!initialized){initialized=true;if(preferences.startMic)setListening(true);}
  }
  function leave(){trace('target:leave');cancelAutoSend();clearPending();reader?.reset(false);revision++;target=null;adapter=null;root?.remove();}
  function afterSend(){
   trace('after-send',{pause:preferences.pauseAfterSend});
   if(preferences.pauseAfterSend&&wanted&&!paused){cancelAutoSend();paused=true;paint();}
  }
  function destroy(){stop();disposed=true;leave();reader?.destroy();controls?.remove();if(session)for(const name of ['onstart','onspeechstart','onspeechend','onresult','onerror','onend','onaudiostart','onaudioend','onsoundstart','onsoundend'])session[name]=null;document.removeEventListener('focusin',focus);document.removeEventListener('input',input);document.removeEventListener('visibilitychange',hidden);window.removeEventListener('pagehide',stop);}
  document.addEventListener('focusin',focus);document.addEventListener('input',input);document.addEventListener('visibilitychange',hidden);window.addEventListener('pagehide',stop);
  return {bind,leave,stop,destroy,showHelp,afterSend,get reader(){return reader;}};
 }
 return {create,parse,resolve,commands,normalize,number,preferences};
})();
