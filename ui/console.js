// console.js v3.140.2423
"use strict";
// Shared controller state and API helpers are defined in app.js.
const detachedConsoles=new Map(),returnedConsoleWindows=new WeakSet(),consoleWindowStops=new WeakMap();
let consoleChannel=null;
try{if(typeof BroadcastChannel==='function')consoleChannel=new BroadcastChannel('oaclis-console-windows');}catch{}
consoleChannel?.addEventListener('message',event=>{
  if(event.data==='discover'&&document.documentElement.dataset.consoleWindow)document.querySelector('.chat-terminal[data-window-active]')?.sendWindowDraft?.();
  else if(event.data?.kind==='oaclis-console-stopped')closeStoppedChatWindows(event.data);
});
window.addEventListener('message',event=>{if(event.origin===location.origin&&event.data?.kind==='oaclis-console-stopped')closeStoppedChatWindows(event.data);});

function closeStoppedChatWindows(value){
  if(typeof value.chat_id!=='string'||!/^\w[\w-]{0,159}$/.test(value.chat_id)||typeof value.device_id!=='string'||!/^\w[\w-]{0,159}$/.test(value.device_id))return;
  for(const child of new Set(detachedConsoles.values())){
    consoleWindowStops.get(child)?.(value);
    if(!child.closed)child.postMessage(value,location.origin);
  }
  document.querySelectorAll('.chat-terminal').forEach(panel=>{
    if(panel.dataset.chatId!==value.chat_id||panel.dataset.deviceId!==value.device_id)return;
    panel.sessionStopped=true;closeChatTerminal(panel,true);
  });
}


function closeChatTerminal(panel,discard=false) {
  if (!panel) return;
  panel.consoleDiscard=discard;
  panel.activeConsole.closed=true;
  panel.sendWindowDraft?.(true);
  panel.cancelSnap?.();
  panel.saveGeometry?.();
  clearTimeout(panel.terminalTimer);
  panel.resetTerminal?.();
  panel.closeImageInput?.();
  panel.jobsStatus?.destroy();
  document.removeEventListener('visibilitychange',panel.terminalVisibility);
  if(panel.windowMessage)window.removeEventListener('message',panel.windowMessage);
  const anchor = panel.opener;
  panel.remove();
  if(document.documentElement.dataset.consoleWindow&&!document.querySelector('.chat-terminal'))window.close();
  window.OaclisLocalVoice?.close(panel);
  if (anchor?.isConnected) anchor.focus({ preventScroll: true });
}

function chooseChatTerminal(connection, anchor) {
  if (connection.operation !== "sync") { openChatTerminal(connection, anchor, connection.target); return; }
  $("#terminal-choice")?.remove();
  const menu = document.createElement("div");
  menu.id = "terminal-choice";
  menu.setAttribute("popover", "manual");
  menu.setAttribute("role", "dialog");
  menu.setAttribute("aria-label", OaclisSettings.text("Dove aprire la chat"));
  menu.innerHTML = OaclisSettings.html(('<button class="icon-button popover-close" aria-label="Chiudi scelta">' + OaclisIcon('close') + '</button><h2>Apri su</h2><div class="terminal-destinations"></div>'));
  document.body.append(menu);
  menu.showPopover();
  positionContext(menu, anchor);
  const close = () => { menu.remove(); if (anchor?.isConnected) anchor.focus(); };
  menu.querySelector(".popover-close").onclick = close;
  for (const id of [connection.source, connection.target]) {
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.deviceId = id;
    button.textContent = deviceName(id) + (id === "master" ? OaclisSettings.text(" · Locale") : OaclisSettings.text(" · Remoto"));
    button.onclick = () => { close(); openChatTerminal(connection, anchor, id); };
    menu.querySelector(".terminal-destinations").append(button);
  }
  menu.querySelector(".terminal-destinations button").focus();
}

function bindConsoleSnap(panel) {
  const modes=['left','right','top-left','top-right','bottom-left','bottom-right','full'];
  let preview=null,zone=null;
  const bounds=mode=>{
    const width=innerWidth-16,height=innerHeight-16,halfWidth=(width-8)/2,halfHeight=(height-8)/2;
    if(mode!=='full'&&(halfWidth<280||mode.includes('-')&&halfHeight<320))mode='full';
    return {mode,left:mode.includes('right')?16+halfWidth:8,top:mode.startsWith('bottom-')?16+halfHeight:8,width:mode==='full'?width:halfWidth,height:mode.includes('-')?halfHeight:height};
  };
  const apply=rect=>{
    panel.style.minWidth='min(280px, calc(100vw - 16px))';panel.style.minHeight='min(320px, calc(100dvh - 16px))';
    panel.style.maxWidth='calc(100vw - 16px)';panel.style.maxHeight='calc(100dvh - 16px)';
    for(const key of ['width','height'])panel.style[key]=Math.min(key==='width'?innerWidth-16:innerHeight-16,rect[key])+'px';
    panel.style.left=Math.max(8,Math.min(rect.left,innerWidth-panel.offsetWidth-8))+'px';
    panel.style.top=Math.max(8,Math.min(rect.top,innerHeight-panel.offsetHeight-8))+'px';panel.dataset.moved='true';
  };
  panel.snapConsole=mode=>{
    if(mode==='restore'){
      if(panel.freeGeometry)apply(panel.freeGeometry);
      delete panel.dataset.snap;panel.freeGeometry=null;
    }else if(modes.includes(mode)){
      if(!panel.dataset.snap)panel.freeGeometry={left:panel.offsetLeft,top:panel.offsetTop,width:panel.offsetWidth,height:panel.offsetHeight};
      const rect=bounds(mode);panel.dataset.snap=rect.mode;apply(rect);
    }else return;
    panel.saveGeometry?.();
  };
  panel.cancelSnap=()=>{preview?.remove();preview=null;zone=null;};
  panel.dragStart=event=>{
    if(!panel.dataset.snap)return;
    const ratio=(event.clientX-panel.offsetLeft)/panel.offsetWidth,top=event.clientY-panel.offsetTop;
    panel.snapConsole('restore');
    apply({left:event.clientX-ratio*panel.offsetWidth,top:event.clientY-top,width:panel.offsetWidth,height:panel.offsetHeight});
  };
  panel.dragMove=event=>{
    const x=event.clientX,y=event.clientY,left=x<=24,right=x>=innerWidth-24,top=y<=24,bottom=y>=innerHeight-24;
    zone=(left||right)&&(y<innerHeight/4||y>innerHeight*3/4)?(y<innerHeight/2?'top-':'bottom-')+(left?'left':'right'):
      (top||bottom)&&(x<innerWidth/4||x>innerWidth*3/4)?(top?'top-':'bottom-')+(x<innerWidth/2?'left':'right'):
      left?'left':right?'right':top?'full':null;
    if(!zone){panel.cancelSnap();return;}
    if(!preview){preview=document.createElement('div');preview.className='console-snap-preview';preview.setAttribute('aria-hidden','true');document.body.append(preview);}
    const rect=bounds(zone);for(const key of ['left','top','width','height'])preview.style[key]=rect[key]+'px';
  };
  panel.dragEnd=event=>{const mode=zone;panel.cancelSnap();if(event.type==='pointerup'&&mode)panel.snapConsole(mode);};
  panel.addEventListener('keydown',event=>{
    if(!event.altKey||event.ctrlKey||event.metaKey||event.shiftKey||!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key))return;
    event.preventDefault();event.stopImmediatePropagation();
    const mode=panel.dataset.snap||'',side=mode.includes('left')?'left':mode.includes('right')?'right':'';
    panel.snapConsole(event.key==='ArrowLeft'||event.key==='ArrowRight'?(mode.includes('-')?mode.split('-')[0]+'-':'')+(event.key==='ArrowLeft'?'left':'right'):
      event.key==='ArrowUp'?(side&&!mode.startsWith('top-')?'top-'+side:'full'):
      side&&!mode.startsWith('bottom-')?'bottom-'+side:'restore');
  },true);
  panel.querySelector('h2').addEventListener('dblclick',event=>{if(!event.target.closest('button'))panel.snapConsole(panel.dataset.snap==='full'?'restore':'full');});
}

function activateConsoleWindow(panel,entry,deviceId){
  if(!document.documentElement.dataset.consoleWindow)return;
  document.querySelectorAll('.chat-terminal').forEach(other=>other.removeAttribute('data-window-active'));
  panel.setAttribute('data-window-active','');document.title=chatLabel(entry)+' · Oaclis';
  history.replaceState(null,'',location.pathname+'#console='+encodeURIComponent(JSON.stringify({id:entry.id,name:chatLabel(entry),device:deviceId})));
}

function trackConsoleWindow(child,connection,anchor,deviceId,draft){
  const key=JSON.stringify([deviceId,connection.entry.id]);let returnConnection=connection;
  const stopped=value=>{if(value.device_id===deviceId&&value.chat_id===returnConnection?.entry.id){returnConnection=null;draft=null;}};
  consoleWindowStops.set(child,stopped);
  const receive=event=>{
    if(event.origin!==location.origin||event.source!==child||event.data?.device_id!==deviceId)return;
    if(event.data.kind==='oaclis-console-stopped'){stopped(event.data);return;}
    if(event.data.kind==='oaclis-console-draft'&&event.data.discard===true){returnConnection=null;draft=null;return;}
    if(!returnConnection)return;
    if(event.data.kind==='oaclis-console-ready'&&event.data.chat_id===returnConnection.entry.id)child.postMessage({kind:'oaclis-console-draft',chat_id:returnConnection.entry.id,device_id:deviceId,draft},location.origin);
    if(event.data.kind==='oaclis-console-draft'&&event.data.active&&typeof event.data.entry?.id==='string'&&/^[\w-]{1,160}$/.test(event.data.entry.id)){draft=event.data.draft;returnConnection={entry:event.data.entry};}
  };
  window.addEventListener('message',receive);detachedConsoles.set(key,child);
  const watch=setInterval(()=>{
    if(!child.closed)return;
    clearInterval(watch);window.removeEventListener('message',receive);if(detachedConsoles.get(key)===child)detachedConsoles.delete(key);if(consoleWindowStops.get(child)===stopped)consoleWindowStops.delete(child);returnedConsoleWindows.add(child);
    if(!returnConnection)return;
    openChatTerminal(returnConnection,anchor?.isConnected?anchor:document.querySelector('#add-device'),deviceId,true);
    [...document.querySelectorAll('.chat-terminal')].find(panel=>panel.dataset.chatId===returnConnection.entry.id&&panel.dataset.deviceId===deviceId)?.restoreDraft(draft);
  },500);
}

function openChatTerminal(connection, anchor, deviceId, embedded=false) {
  const windowKey=JSON.stringify([deviceId,connection.entry.id]),detached=detachedConsoles.get(windowKey);
  if(detached&&!detached.closed){detached.focus();return;}
  const existing = [...document.querySelectorAll(".chat-terminal")].find(p =>
    p.dataset.chatId === connection.entry.id && p.dataset.deviceId === deviceId);
  if (existing) {
    activateConsoleWindow(existing,connection.entry,deviceId);
    existing.hidePopover(); existing.showPopover();
    window.OaclisLocalVoice?.activate(existing);
    existing.querySelector("textarea").focus();
    existing.openTerminal?.();
    return;
  }
  const panel = OaclisTerminal.createConsole();
  panel.classList.add("chat-terminal");
  activateConsoleWindow(panel,connection.entry,deviceId);
  panel.dataset.chatId = connection.entry.id;
  panel.dataset.deviceId = deviceId;
  panel.opener = anchor;
  panel.setAttribute("popover", "manual");
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", OaclisSettings.text("Console Codex · ") + deviceName(deviceId));
  const chatIcon=OaclisTerminal.chatIcon({...connection.entry,...appearance(connection.entry),name:chatLabel(connection.entry)});
  const pane=OaclisTerminal.createPane({id:connection.entry.id,name:chatLabel(connection.entry)+' · '+deviceName(deviceId),icon:chatIcon},(action,body)=>action==='stop'?stopConsole():Object.hasOwn(body,'text')?submit():send({action,...body}));
  const chat=pane.console;panel.activeConsole=chat;panel.querySelector('.console-panes').append(pane);
  const close=document.createElement('button');close.type='button';close.className='icon-button popover-close';close.setAttribute('aria-label',OaclisSettings.text('Chiudi chat'));close.innerHTML=OaclisIcon('close');panel.querySelector('h2').append(close);
  document.body.append(panel);
  panel.showPopover();
  if(document.documentElement.dataset.consoleWindow){
    panel.classList.add('context-popover','resizable-panel');
    if(window.opener&&innerWidth===858)window.resizeBy(28,0);
  }
  else {
  bindPanelDrag(panel);
  positionContext(panel, anchor);
  const count = document.querySelectorAll(".chat-terminal").length - 1;
  panel.style.left = Math.max(8, Math.min(parseFloat(panel.style.left) + count * 24, innerWidth - panel.offsetWidth - 8)) + "px";
  panel.style.top = Math.max(8, Math.min(parseFloat(panel.style.top) + count * 24, innerHeight - panel.offsetHeight - 8)) + "px";
  const geometryKey='oaclis-console:'+JSON.stringify([deviceId,connection.entry.id]);
  try{
    const saved=JSON.parse(localStorage.getItem(geometryKey)||'null');
    if(saved&&['left','top','width','height'].every(key=>Number.isFinite(saved[key]))&&saved.width>0&&saved.height>0){
      panel.style.maxWidth='calc(100vw - 16px)';panel.style.maxHeight='calc(100dvh - 16px)';
      const width=!saved.snap&&saved.width===842?panel.offsetWidth:saved.width;
      panel.style.width=Math.min(innerWidth-16,Math.max(280,width))+'px';
      panel.style.height=Math.min(innerHeight-16,Math.max(320,saved.height))+'px';
      panel.style.left=Math.max(8,Math.min(saved.left,innerWidth-panel.offsetWidth-8))+'px';
      panel.style.top=Math.max(8,Math.min(saved.top,innerHeight-panel.offsetHeight-8))+'px';panel.dataset.moved='true';
      if(['left','right','top-left','top-right','bottom-left','bottom-right','full'].includes(saved.snap))panel.dataset.snap=saved.snap;
      if(saved.restore&&['left','top','width','height'].every(key=>Number.isFinite(saved.restore[key]))&&saved.restore.width>0&&saved.restore.height>0)panel.freeGeometry=saved.restore;
    }
  }catch{}
  let savedGeometry='';
  panel.saveGeometry=()=>{
    if(!panel.isConnected||!panel.matches(':popover-open'))return;
    const value=JSON.stringify({left:panel.offsetLeft,top:panel.offsetTop,width:panel.offsetWidth,height:panel.offsetHeight,snap:panel.dataset.snap||null,restore:panel.freeGeometry||null});
    if(value===savedGeometry)return;
    try{localStorage.setItem(geometryKey,value);savedGeometry=value;}catch{}
  };
  bindConsoleSnap(panel);
  if(panel.dataset.snap)panel.snapConsole(panel.dataset.snap);
  panel.addEventListener('pointerup',panel.saveGeometry);
  panel.addEventListener('keyup',event=>{if(event.target.matches('h2'))panel.saveGeometry();});
  }
  panel.addEventListener("click",event=>{
    if(!panel.isConnected||event.target.closest(".console-send-menu")||panel.querySelector(".console-send-menu:popover-open"))return;
    const focus=event.target.closest("textarea,input,button,select,pre,h2")||panel.querySelector("textarea");
    panel.dataset.raised="true";panel.hidePopover();panel.showPopover();
    focus.focus({preventScroll:true});window.OaclisLocalVoice?.activate(panel);
  },true);
  panel.querySelector(".popover-close").onclick = () => closeChatTerminal(panel);
  if(!document.documentElement.dataset.consoleWindow){
    const detach=document.createElement('button');detach.type='button';detach.className='icon-button console-detach';detach.title=OaclisSettings.text('Apri in una finestra separata');detach.setAttribute('aria-label',detach.title);detach.innerHTML=OaclisIcon('externalLink');
    detach.onclick=()=>{
      if(submitting||chat.preparingImage)return;
      const url=new URL(location.pathname,location.origin);url.hash='console='+encodeURIComponent(JSON.stringify({id:connection.entry.id,name:chatLabel(connection.entry),device:deviceId}));
      const child=window.open(url,'oaclis-console-'+deviceId+'-'+connection.entry.id,'popup=yes,width='+Math.min(window.screen.availWidth,Math.max(480,panel.offsetWidth+16))+',height='+Math.min(window.screen.availHeight,Math.max(400,panel.offsetHeight+16)));
      if(!child){notice(OaclisSettings.text('Consenti l’apertura della finestra nel browser.'));return;}
      try{localStorage.setItem('oaclis-console-mode:'+windowKey,'window');}catch{}
      trackConsoleWindow(child,connection,anchor,deviceId,panel.consoleDraft());child.focus();closeChatTerminal(panel);
    };
    panel.querySelector('.popover-close').before(detach);
  }
  const screen = panel.querySelector("pre"), form = panel.querySelector("form");
  const payload = { device_id: deviceId, chat_id: connection.entry.id };
  const status = document.createElement("small"); status.className = "terminal-state"; status.setAttribute("role", "status"); panel.querySelector("h2").append(status);
  status.addEventListener("mouseenter",()=>{status.title=status.textContent;});
  const jobs=document.createElement("span");panel.querySelector("h2").append(jobs);panel.jobsStatus=OaclisJobs.inline(jobs);
  let stopping=false,busy=false,ready=false,submitting=false,waitingInputs=0,idle=Promise.resolve();
  panel.resetTerminal=()=>{
    clearTimeout(panel.terminalTimer);ready=false;chat.reset();
    if(!panel.sessionStopped)fetch('/api/terminal',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({...payload,action:'reset'}),keepalive:true}).catch(()=>{});
  };
  const send = async value => {
    if (value.action !== "read") status.textContent = OaclisSettings.text("Ricevuto");
    if (value.action === "read") { if (busy || waitingInputs || document.documentElement.dataset.consoleWindow&&!panel.hasAttribute("data-window-active")) return false; }
    else { waitingInputs++; while (busy) await idle; waitingInputs--; }
    if (!panel.isConnected) return false;
    busy = true;
    let done; idle = new Promise(resolve => { done = resolve; });
    try {
      if (value.action !== "read") status.textContent = OaclisSettings.text("Operazione in corso");
      const stamp = window.OaclisLocalVoice?.stamp(panel);
      const result = await api("terminal", { ...payload, ...(value.action==="read"&&chat.revision?{screen_revision:chat.revision}:{}), ...value, ...window.OaclisLocalVoice?.request(panel) });
      if (!panel.isConnected) return false;
      if (result.stopped === true) {
        const event={kind:'oaclis-console-stopped',chat_id:connection.entry.id,device_id:deviceId};
        consoleChannel?.postMessage(event);window.opener?.postMessage(event,location.origin);closeStoppedChatWindows(event);
        return false;
      }
      panel.jobsStatus.update(result.jobs?.map(job=>job.device_id==='master'&&deviceId!=='master'?{...job,device_id:deviceId}:job));
      chat.render(result,value.action);
      window.OaclisLocalVoice?.update(panel,result,stamp,typeof value.text === "string");
      if (value.action !== "read" || status.dataset.error) status.textContent=OaclisSettings.text(value.action === "input" ? "Inviato" : "Collegato");
      delete status.dataset.error;
      ready = true;
      form.querySelectorAll("textarea,button").forEach(e => e.disabled = false);
      return true;
    } catch (error) { if (panel.isConnected) {panel.jobsStatus.stale();status.textContent=OaclisSettings.text(value.action === "input" ? "Invio non confermato" : "Collegamento temporaneamente non disponibile. Riprova.");status.dataset.error="true";} if (value.action === "open") ready = false; console.error(error);throw error; }
    finally { busy = false; done(); }
  };
  let terminalPolling=false;
  const pollTerminal = async () => {
    if (document.hidden || terminalPolling || !panel.isConnected) return;
    terminalPolling=true;
    try { await send({ action: ready ? "read" : "open" }); }
    catch { /* send() already displays the connection error; keep observing the session. */ }
    finally {
      terminalPolling=false;clearTimeout(panel.terminalTimer);
      if (panel.isConnected && !document.hidden) panel.terminalTimer=setTimeout(pollTerminal,1500);
    }
  };
  panel.terminalVisibility=()=>{clearTimeout(panel.terminalTimer);if(!document.hidden)pollTerminal();};
  document.addEventListener('visibilitychange',panel.terminalVisibility);
  const submit = async () => {
    if (!ready || submitting || chat.preparingImage) return false;
    const text = form.elements.text.value, attachment=chat.image; submitting = true;
    try {
      const sent=await send({action:"input",text,key:"Enter",...(attachment?{image:attachment}:{})});
      if(sent){if(chat.image===attachment)files.remove();form.elements.text.value=OaclisTerminal.remainingDraft(form.elements.text.value,text);panel.sendWindowDraft?.();window.OaclisLocalVoice?.sent(panel);}
      return sent;
    } finally {submitting=false;}
  };
  const files=panel.files=OaclisTerminal.attachImages(panel,()=>chat,()=>panel.sendWindowDraft?.());
  panel.closeImageInput=files.destroy;
  panel.consoleDraft=()=>({text:form.elements.text.value,image:chat.image});
  panel.restoreDraft=draft=>{
    if(typeof draft?.text==='string'&&draft.text.length<=8000)form.elements.text.value=draft.text;
    files.set(typeof draft?.image==='string'&&draft.image.startsWith('data:image/jpeg;base64,')&&draft.image.length<=699075?draft.image:null);
  };
  if(document.documentElement.dataset.consoleWindow){
    panel.sendWindowDraft=(returning=false)=>window.opener?.postMessage({kind:'oaclis-console-draft',returning:returning===true,discard:panel.consoleDiscard===true,chat_id:connection.entry.id,device_id:deviceId,active:panel.hasAttribute('data-window-active'),entry:{id:connection.entry.id,name:chatLabel(connection.entry)},draft:panel.consoleDraft()},location.origin);
    const receive=event=>{if(event.origin===location.origin&&event.source===window.opener&&event.data?.kind==='oaclis-console-draft'&&event.data.chat_id===connection.entry.id&&event.data.device_id===deviceId)panel.restoreDraft(event.data.draft);};
    window.addEventListener('message',receive);panel.windowMessage=receive;form.elements.text.addEventListener('input',panel.sendWindowDraft);
    window.opener?.postMessage({kind:'oaclis-console-ready',chat_id:connection.entry.id,device_id:deviceId},location.origin);
  }
  panel.voiceAdapter={chatId:connection.entry.id,compose:form.elements.text,view:panel};
  const stopConsole=async()=>{
    if(!ready||stopping)return;stopping=true;
    try{if(await send({action:'stop'})){
      const value={kind:'oaclis-console-stopped',chat_id:connection.entry.id,device_id:deviceId};
      consoleChannel?.postMessage(value);window.opener?.postMessage(value,location.origin);closeStoppedChatWindows(value);
    }}
    finally{stopping=false;}
  };
  panel.openTerminal=async()=>{
    if(ready||busy)return;
    try{if(await send({action:'open'}))form.elements.text.focus();}
    catch { /* send() displays the connection error; polling retries the opening. */ }
    finally{pollTerminal();}
  };
  window.OaclisLocalVoice?.activate(panel);
  try{if(!embedded&&!document.documentElement.dataset.consoleWindow&&localStorage.getItem('oaclis-console-mode:'+windowKey)==='window')panel.querySelector('.console-detach').click();}catch{}
  if(panel.isConnected)panel.openTerminal();
}

window.addEventListener('pagehide',()=>document.querySelectorAll('.chat-terminal').forEach(panel=>{panel.sendWindowDraft?.(true);panel.saveGeometry?.();panel.resetTerminal?.();}));

window.addEventListener('message',event=>{
  const value=event.data;
  if(document.documentElement.dataset.consoleWindow||event.origin!==location.origin||!event.source||event.source===window||returnedConsoleWindows.has(event.source)||[...detachedConsoles.values()].includes(event.source)||value?.kind!=='oaclis-console-draft'||!value.active||typeof value.entry?.id!=='string'||!/^[\w-]{1,160}$/.test(value.entry.id)||typeof value.device_id!=='string'||!/^[\w-]{1,160}$/.test(value.device_id))return;
  const entry={id:value.entry.id,name:typeof value.entry.name==='string'?value.entry.name.slice(0,160):'Chat'};
  trackConsoleWindow(event.source,{entry},document.querySelector('#add-device'),value.device_id,value.draft);
});
