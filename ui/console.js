// console.js v3.122.2150
"use strict";
// Shared controller state and API helpers are defined in app.js.
const detachedConsoles=new Map();

function closeChatTerminal(panel) {
  if (!panel) return;
  panel.sendWindowDraft?.(true);
  panel.cancelSnap?.();
  panel.saveGeometry?.();
  clearTimeout(panel.terminalTimer);
  panel.closeSendMenu?.();
  document.removeEventListener('visibilitychange',panel.terminalVisibility);
  if(panel.windowMessage)window.removeEventListener('message',panel.windowMessage);
  const anchor = panel.opener;
  panel.remove();
  if(document.documentElement.dataset.consoleWindow)window.close();
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
  const panel = document.createElement("section");
  panel.className = "chat-terminal";
  activateConsoleWindow(panel,connection.entry,deviceId);
  panel.dataset.chatId = connection.entry.id;
  panel.dataset.deviceId = deviceId;
  panel.opener = anchor;
  panel.setAttribute("popover", "manual");
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", OaclisSettings.text("Console Codex · ") + deviceName(deviceId));
  panel.innerHTML = OaclisSettings.html(('<button class="icon-button popover-close" aria-label="Chiudi chat">' + OaclisIcon('close') + '</button><h2><span class="terminal-title">')) + esc(chatLabel(connection.entry)) + ' · ' + esc(deviceName(deviceId)) + OaclisSettings.html('</span></h2><pre class="terminal-screen" tabindex="0" aria-label="Schermo Codex">Connessione…</pre><form class="terminal-input"><div class="console-draft"><div class="console-attachment" hidden><img alt=""><button type="button" aria-label="Rimuovi immagine">' + OaclisIcon('close') + '</button></div><textarea name="text" maxlength="8000" aria-label="Messaggio a Codex" rows="2" placeholder="Scrivi a Codex" disabled></textarea></div><button type="submit" class="icon-button" aria-label="Invia messaggio" aria-haspopup="menu" aria-expanded="false" disabled>' + OaclisIcon('arrowUp') + '</button></form><div class="terminal-keys"></div>');
  panel.querySelector("h2").insertAdjacentHTML("afterbegin",badge(connection.entry));
  panel.querySelector("h2 .chat-badge").style.color=appearance(connection.entry).color||"var(--ink)";
  document.body.append(panel);
  panel.showPopover();
  if(document.documentElement.dataset.consoleWindow)panel.classList.add('context-popover','resizable-panel');
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
      panel.style.width=Math.min(innerWidth-16,Math.max(280,saved.width))+'px';
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
      if(submitting||preparingImage)return;
      const url=new URL(location.pathname,location.origin);url.hash='console='+encodeURIComponent(JSON.stringify({id:connection.entry.id,name:chatLabel(connection.entry),device:deviceId}));
      const child=window.open(url,'oaclis-console-'+deviceId+'-'+connection.entry.id,'popup=yes,width='+Math.min(window.screen.availWidth,Math.max(480,panel.offsetWidth+16))+',height='+Math.min(window.screen.availHeight,Math.max(400,panel.offsetHeight+16)));
      if(!child){notice(OaclisSettings.text('Consenti l’apertura della finestra nel browser.'));return;}
      let draft=panel.consoleDraft(),returnConnection=connection;
      const receive=event=>{
        if(event.origin!==location.origin||event.source!==child||event.data?.device_id!==deviceId)return;
        if(event.data.kind==='oaclis-console-ready'&&event.data.chat_id===returnConnection.entry.id)child.postMessage({kind:'oaclis-console-draft',chat_id:returnConnection.entry.id,device_id:deviceId,draft},location.origin);
        if(event.data.kind==='oaclis-console-draft'&&event.data.active&&typeof event.data.entry?.id==='string'&&/^[\w-]{1,160}$/.test(event.data.entry.id)){draft=event.data.draft;returnConnection={entry:event.data.entry};}
      };
      try{localStorage.setItem('oaclis-console-mode:'+windowKey,'window');}catch{}
      window.addEventListener('message',receive);detachedConsoles.set(windowKey,child);child.focus();closeChatTerminal(panel);
      const watch=setInterval(()=>{
        if(!child.closed)return;
        clearInterval(watch);window.removeEventListener('message',receive);detachedConsoles.delete(windowKey);
        openChatTerminal(returnConnection,anchor?.isConnected?anchor:document.querySelector('#add-device'),deviceId,true);
        [...document.querySelectorAll('.chat-terminal')].find(p=>p.dataset.chatId===returnConnection.entry.id&&p.dataset.deviceId===deviceId)?.restoreDraft(draft);
      },500);
    };
    panel.querySelector('.popover-close').before(detach);
  }
  const screen = panel.querySelector("pre"), form = panel.querySelector("form");
  const payload = { device_id: deviceId, chat_id: connection.entry.id };
  const status = document.createElement("small"); status.className = "terminal-state"; status.setAttribute("role", "status"); panel.querySelector("h2").append(status);
  status.addEventListener("mouseenter",()=>{status.title=status.textContent;});
  let followOutput = true;
  screen.addEventListener("scroll", () => { followOutput = screen.scrollHeight-screen.clientHeight-screen.scrollTop <= 24; }, {passive:true});
  let busy = false, ready = false, submitting = false, preparingImage = false, image = null, waitingInputs = 0, idle = Promise.resolve();
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
      const result = await api("terminal", { ...payload, ...value, ...window.OaclisLocalVoice?.request(panel) });
      if (!panel.isConnected) return false;
      const top=screen.scrollTop,left=screen.scrollLeft;
      OaclisTerminal.render(screen,OaclisTerminal.consoleScreen(result.screen),result.screen_ansi);
      if (value.action === "input" || value.action === "open") followOutput = true;
      if (followOutput) screen.scrollTop = screen.scrollHeight;
      else {screen.scrollTop=top;screen.scrollLeft=left;}
      window.OaclisLocalVoice?.update(panel,result,stamp,typeof value.text === "string");
      if (value.action !== "read" || status.dataset.error) status.textContent=OaclisSettings.text(value.action === "input" ? "Inviato" : "Collegato");
      delete status.dataset.error;
      ready = true;
      form.querySelectorAll("textarea,button").forEach(e => e.disabled = false);
      return true;
    } catch (error) { if (panel.isConnected) {status.textContent=OaclisSettings.text(value.action === "input" ? "Invio non confermato" : "Collegamento temporaneamente non disponibile. Riprova.");status.dataset.error="true";} if (value.action === "open") ready = false; return false; }
    finally { busy = false; done(); }
  };
  let terminalPolling=false;
  const pollTerminal = async () => {
    if (document.hidden || terminalPolling || !panel.isConnected || !ready) return;
    terminalPolling=true;
    try { await send({ action: "read" }); }
    finally {
      terminalPolling=false;clearTimeout(panel.terminalTimer);
      if (panel.isConnected && ready && !document.hidden) panel.terminalTimer=setTimeout(pollTerminal,1500);
    }
  };
  panel.terminalVisibility=()=>{clearTimeout(panel.terminalTimer);if(!document.hidden)pollTerminal();};
  document.addEventListener('visibilitychange',panel.terminalVisibility);
  const submit = async () => {
    if (!ready || submitting || preparingImage) return false;
    const text = form.elements.text.value, attachment=image; submitting = true;
    try {
      const sent=await send({action:"input",text,key:"Enter",...(attachment?{image:attachment}:{})});
      if(sent){if(image===attachment)removeImage();if(form.elements.text.value===text)form.elements.text.value="";panel.sendWindowDraft?.();window.OaclisLocalVoice?.sent(panel);}
      return sent;
    } finally {submitting=false;}
  };
  form.onsubmit=guard(async event=>{event.preventDefault();await submit();});
  const sendButton=form.querySelector('button[type="submit"]'),preview=form.querySelector('.console-attachment');
  const removeImage=()=>{image=null;preview.hidden=true;preview.querySelector('img').removeAttribute('src');panel.sendWindowDraft?.();};
  preview.querySelector('button').onclick=removeImage;
  panel.consoleDraft=()=>({text:form.elements.text.value,image});
  panel.restoreDraft=draft=>{
    if(typeof draft?.text==='string'&&draft.text.length<=8000)form.elements.text.value=draft.text;
    if(typeof draft?.image==='string'&&draft.image.startsWith('data:image/jpeg;base64,')&&draft.image.length<=699075){image=draft.image;preview.querySelector('img').src=image;preview.hidden=false;}
    else removeImage();
  };
  if(document.documentElement.dataset.consoleWindow){
    panel.sendWindowDraft=(returning=false)=>window.opener?.postMessage({kind:'oaclis-console-draft',returning:returning===true,chat_id:connection.entry.id,device_id:deviceId,active:panel.hasAttribute('data-window-active'),entry:{id:connection.entry.id,name:chatLabel(connection.entry)},draft:panel.consoleDraft()},location.origin);
    const receive=event=>{if(event.origin===location.origin&&event.source===window.opener&&event.data?.kind==='oaclis-console-draft'&&event.data.chat_id===connection.entry.id&&event.data.device_id===deviceId)panel.restoreDraft(event.data.draft);};
    window.addEventListener('message',receive);panel.windowMessage=receive;form.elements.text.addEventListener('input',panel.sendWindowDraft);
    window.opener?.postMessage({kind:'oaclis-console-ready',chat_id:connection.entry.id,device_id:deviceId},location.origin);
  }
  const pickImage=camera=>{
    if(submitting||preparingImage)return;
    if(navigator.userActivation&&!navigator.userActivation.isActive){panel.closeSendMenu.show(camera?0:1);return;}
    const input=document.createElement('input');input.type='file';input.accept='image/*';if(camera)input.setAttribute('capture','environment');
    input.onchange=async()=>{
      const file=input.files?.[0];if(!file||!panel.isConnected)return;
      preparingImage=true;sendButton.disabled=true;
      try{const result=await OaclisTerminal.imageData(file);if(panel.isConnected){image=result;preview.querySelector('img').src=image;preview.hidden=false;panel.sendWindowDraft?.();}}
      catch{if(panel.isConnected)notice(OaclisSettings.text('Impossibile caricare l’immagine. Prova un’altra foto.'));}
      finally{preparingImage=false;if(panel.isConnected)sendButton.disabled=!ready;}
    };input.click();
  };
  panel.closeSendMenu=OaclisTerminal.attachSendMenu(form,sendButton,{pickImage,submit,closed:()=>!panel.isConnected});
  panel.voiceAdapter={chatId:connection.entry.id,compose:form.elements.text,send:submit,sending:()=>submitting||preparingImage,takePhoto:()=>pickImage(true),pickImage:()=>pickImage(false),removeImage,hasAttachment:()=>!!image};
  form.elements.text.onkeydown = event => {
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) { event.preventDefault(); form.requestSubmit(); }
  };
  for (const [key, label] of [["Escape", "Esc"], ["1", "1"], ["2", "2"], ["3", "3"], ["C-c", "Ctrl+C"]]) {
    const button = document.createElement("button");
    button.type = "button"; button.className = "icon-button";
    button.textContent=label;button.dataset.key=key;button.setAttribute("aria-label",label);
    button.onclick=()=>{if(ready)send({action:"input",key});};
    panel.querySelector(".terminal-keys").append(button);
  }
  screen.onkeydown = e => {
    const keys = { ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right", Enter: "Enter", Tab: "Tab", Backspace: "BSpace" };
    if (ready && keys[e.key]) { e.preventDefault(); send({ action: "input", key: keys[e.key] }); }
  };
  panel.openTerminal=async()=>{
    if(ready||busy)return;
    if(await send({action:'open'})){
      if(!panel.isConnected)return;
      form.elements.text.focus();pollTerminal();
    }
  };
  window.OaclisLocalVoice?.activate(panel);
  try{if(!embedded&&!document.documentElement.dataset.consoleWindow&&localStorage.getItem('oaclis-console-mode:'+windowKey)==='window')panel.querySelector('.console-detach').click();}catch{}
  if(panel.isConnected)panel.openTerminal();
}

window.addEventListener('pagehide',()=>document.querySelectorAll('.chat-terminal').forEach(panel=>{panel.sendWindowDraft?.(true);panel.saveGeometry?.();}));

window.addEventListener('message',event=>{
  const value=event.data;
  if(document.documentElement.dataset.consoleWindow||event.origin!==location.origin||!event.source||event.source===window||[...detachedConsoles.values()].includes(event.source)||value?.kind!=='oaclis-console-draft'||!value.returning||!value.active||typeof value.entry?.id!=='string'||!/^[\w-]{1,160}$/.test(value.entry.id)||typeof value.device_id!=='string'||!/^[\w-]{1,160}$/.test(value.device_id))return;
  setTimeout(()=>{if(!event.source.closed)return;const entry={id:value.entry.id,name:typeof value.entry.name==='string'?value.entry.name.slice(0,160):'Chat'};openChatTerminal({entry},document.querySelector('#add-device'),value.device_id,true);[...document.querySelectorAll('.chat-terminal')].find(panel=>panel.dataset.chatId===entry.id&&panel.dataset.deviceId===value.device_id)?.restoreDraft(value.draft);},500);
});
