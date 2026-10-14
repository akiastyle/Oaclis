// console.js v2.95.1769
"use strict";
// Shared controller state and API helpers are defined in app.js.

function closeChatTerminal(panel) {
  if (!panel) return;
  clearTimeout(panel.terminalTimer);
  const anchor = panel.opener;
  panel.remove();
  if (anchor?.isConnected) anchor.focus({ preventScroll: true });
}

function chooseChatTerminal(connection, anchor) {
  if (connection.operation !== "sync") { openChatTerminal(connection, anchor, connection.target); return; }
  $("#terminal-choice")?.remove();
  const menu = document.createElement("div");
  menu.id = "terminal-choice";
  menu.setAttribute("popover", "manual");
  menu.setAttribute("role", "dialog");
  menu.setAttribute("aria-label", "Dove aprire la chat");
  menu.innerHTML = '<button class="icon-button popover-close" aria-label="Chiudi scelta"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg></button><h2>Apri su</h2><div class="terminal-destinations"></div>';
  document.body.append(menu);
  menu.showPopover();
  positionContext(menu, anchor);
  const close = () => { menu.remove(); if (anchor?.isConnected) anchor.focus(); };
  menu.querySelector(".popover-close").onclick = close;
  for (const id of [connection.source, connection.target]) {
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.deviceId = id;
    button.textContent = deviceName(id) + (id === "master" ? " · Locale" : " · Remoto");
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
    existing.querySelector("textarea").focus();
    return;
  }
  const panel = document.createElement("section");
  panel.className = "chat-terminal";
  panel.dataset.chatId = connection.entry.id;
  panel.dataset.deviceId = deviceId;
  panel.opener = anchor;
  panel.setAttribute("popover", "manual");
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", "Console Codex · " + deviceName(deviceId));
  panel.innerHTML = '<button class="icon-button popover-close" aria-label="Chiudi chat"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg></button><h2>' + esc(chatLabel(connection.entry)) + ' · ' + esc(deviceName(deviceId)) + '</h2><pre class="terminal-screen" tabindex="0" aria-label="Schermo Codex">Connessione…</pre><form class="terminal-input"><textarea name="text" maxlength="8000" aria-label="Messaggio a Codex" rows="2" placeholder="Scrivi a Codex" disabled></textarea></form><div class="terminal-keys"></div>';
  document.body.append(panel);
  panel.showPopover();
  positionContext(panel, anchor);
  const count = document.querySelectorAll(".chat-terminal").length - 1;
  panel.style.left = Math.max(8, Math.min(parseFloat(panel.style.left) + count * 24, innerWidth - panel.offsetWidth - 8)) + "px";
  panel.style.top = Math.max(8, Math.min(parseFloat(panel.style.top) + count * 24, innerHeight - panel.offsetHeight - 8)) + "px";
  bindPanelDrag(panel, true);
  panel.querySelector(".popover-close").onclick = () => closeChatTerminal(panel);
  const screen = panel.querySelector("pre"), form = panel.querySelector("form");
  const payload = { device_id: deviceId, chat_id: connection.entry.id };
  let busy = false, ready = false;
  const send = async value => {
    if (busy || !panel.isConnected) return false;
    busy = true;
    try {
      const result = await api("terminal", { ...payload, ...value });
      if (!panel.isConnected) return false;
      screen.textContent = result.screen;
      screen.scrollTop = screen.scrollHeight;
      ready = true;
      form.querySelectorAll("textarea,button").forEach(e => e.disabled = false);
      return true;
    } catch (error) { if (panel.isConnected) screen.textContent = error.message; ready = false; return false; }
    finally { busy = false; }
  };
  const pollTerminal = async () => {
    if (!panel.isConnected || !ready) return;
    await send({ action: "read" });
    if (panel.isConnected && ready) panel.terminalTimer = setTimeout(pollTerminal, 1500);
  };
  form.onsubmit = guard(async e => {
    e.preventDefault();
    if (ready && await send({ action: "input", text: form.elements.text.value, key: "Enter" })) form.elements.text.value = "";
  });
  form.elements.text.onkeydown = event => {
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) { event.preventDefault(); form.requestSubmit(); }
  };
  for (const [key, label] of [["Enter", "↵"], ["Escape", "Esc"], ["Tab", "Tab"], ["Up", "↑"], ["Down", "↓"], ["C-c", "Ctrl+C"]]) {
    const button = document.createElement("button");
    button.type = "button"; button.className = "icon-button"; button.textContent = label;
    button.setAttribute("aria-label", key === "Enter" ? "Invio" : label);
    button.onclick = () => {
      if (!ready) return;
      if (key === "Enter" && form.elements.text.value) form.requestSubmit();
      else send({ action: "input", key });
    };
    panel.querySelector(".terminal-keys").append(button);
  }
  screen.onkeydown = e => {
    const keys = { ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right", Enter: "Enter", Tab: "Tab", Backspace: "BSpace" };
    if (ready && keys[e.key]) { e.preventDefault(); send({ action: "input", key: keys[e.key] }); }
  };
  send({ action: "open" }).then(ok => { if (ok && panel.isConnected) { form.elements.text.focus(); pollTerminal(); } });
}
