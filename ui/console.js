// console.js v2.106.1977
"use strict";
// Shared controller state and API helpers are defined in app.js.

function closeChatTerminal(panel) {
  if (!panel) return;
  clearTimeout(panel.terminalTimer);
  panel.closeSendMenu?.();
  document.removeEventListener('visibilitychange',panel.terminalVisibility);
  const anchor = panel.opener;
  panel.remove();
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

function openChatTerminal(connection, anchor, deviceId) {
  const existing = [...document.querySelectorAll(".chat-terminal")].find(p =>
    p.dataset.chatId === connection.entry.id && p.dataset.deviceId === deviceId);
  if (existing) {
    existing.hidePopover(); existing.showPopover();
    window.OaclisLocalVoice?.activate(existing);
    existing.querySelector("textarea").focus();
    existing.openTerminal?.();
    return;
  }
  const panel = document.createElement("section");
  panel.className = "chat-terminal";
  panel.dataset.chatId = connection.entry.id;
  panel.dataset.deviceId = deviceId;
  panel.opener = anchor;
  panel.setAttribute("popover", "manual");
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", OaclisSettings.text("Console Codex · ") + deviceName(deviceId));
  panel.innerHTML = OaclisSettings.html(('<button class="icon-button popover-close" aria-label="Chiudi chat">' + OaclisIcon('close') + '</button><h2>')) + esc(chatLabel(connection.entry)) + ' · ' + esc(deviceName(deviceId)) + OaclisSettings.html('</h2><pre class="terminal-screen" tabindex="0" aria-label="Schermo Codex">Connessione…</pre><form class="terminal-input"><div class="console-draft"><div class="console-attachment" hidden><img alt=""><button type="button" aria-label="Rimuovi immagine">' + OaclisIcon('close') + '</button></div><textarea name="text" maxlength="8000" aria-label="Messaggio a Codex" rows="2" placeholder="Scrivi a Codex" disabled></textarea></div><button type="submit" class="icon-button" aria-label="Invia messaggio" aria-haspopup="menu" aria-expanded="false" disabled>' + OaclisIcon('arrowUp') + '</button></form><div class="terminal-keys"></div>');
  panel.querySelector("h2").insertAdjacentHTML("afterbegin",badge(connection.entry));
  panel.querySelector("h2 .chat-badge").style.color=appearance(connection.entry).color||"var(--ink)";
  document.body.append(panel);
  panel.showPopover();
  positionContext(panel, anchor);
  const count = document.querySelectorAll(".chat-terminal").length - 1;
  panel.style.left = Math.max(8, Math.min(parseFloat(panel.style.left) + count * 24, innerWidth - panel.offsetWidth - 8)) + "px";
  panel.style.top = Math.max(8, Math.min(parseFloat(panel.style.top) + count * 24, innerHeight - panel.offsetHeight - 8)) + "px";
  bindPanelDrag(panel, true);
  panel.addEventListener("click",event=>{
    if(!panel.isConnected||event.target.closest(".console-send-menu")||panel.querySelector(".console-send-menu:popover-open"))return;
    const focus=event.target.closest("textarea,input,button,select,pre,h2")||panel.querySelector("textarea");
    panel.hidePopover();panel.showPopover();
    focus.focus({preventScroll:true});window.OaclisLocalVoice?.activate(panel);
  },true);
  panel.querySelector(".popover-close").onclick = () => closeChatTerminal(panel);
  const screen = panel.querySelector("pre"), form = panel.querySelector("form");
  const payload = { device_id: deviceId, chat_id: connection.entry.id };
  const status = document.createElement("small"); status.className = "terminal-state"; status.setAttribute("role", "status"); panel.querySelector("h2").after(status);
  let followOutput = true;
  screen.addEventListener("scroll", () => { followOutput = screen.scrollHeight-screen.clientHeight-screen.scrollTop <= 24; }, {passive:true});
  let busy = false, ready = false, submitting = false, preparingImage = false, image = null, waitingInputs = 0, idle = Promise.resolve();
  const send = async value => {
    if (value.action !== "read") status.textContent = OaclisSettings.text("Ricevuto");
    if (value.action === "read") { if (busy || waitingInputs) return false; }
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
      if(sent){if(image===attachment)removeImage();if(form.elements.text.value===text)form.elements.text.value="";window.OaclisLocalVoice?.sent(panel);}
      return sent;
    } finally {submitting=false;}
  };
  form.onsubmit=guard(async event=>{event.preventDefault();await submit();});
  const sendButton=form.querySelector('button[type="submit"]'),preview=form.querySelector('.console-attachment');
  const removeImage=()=>{image=null;preview.hidden=true;preview.querySelector('img').removeAttribute('src');};
  preview.querySelector('button').onclick=removeImage;
  const pickImage=camera=>{
    if(submitting||preparingImage)return;
    if(navigator.userActivation&&!navigator.userActivation.isActive){panel.closeSendMenu.show(camera?0:1);return;}
    const input=document.createElement('input');input.type='file';input.accept='image/*';if(camera)input.setAttribute('capture','environment');
    input.onchange=async()=>{
      const file=input.files?.[0];if(!file||!panel.isConnected)return;
      preparingImage=true;sendButton.disabled=true;
      try{const result=await OaclisTerminal.imageData(file);if(panel.isConnected){image=result;preview.querySelector('img').src=image;preview.hidden=false;}}
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
  panel.openTerminal();
}
