// panels.js v3.138.2538
"use strict";
// Shared controller state and API helpers are defined in app.js.

function openPanel() {
  $("#panel").hidden = false;
  if (!$("#panel").matches(":popover-open")) $("#panel").showPopover();
}

function showLibrary() {
  closeConnectionPopover();
  selected = null;
  $("#details").hidden = true;
  $("#library").hidden = false;
  $("#back").hidden = true;
  $("#refresh").hidden = false;
  $("#edit-device").hidden = sourceId === "master";
  renderLibrary();
}

function closePanel() {
  closeConnectionPopover();
  const origin = sourceId;
  catalogRequest++;
  if ($("#panel").matches(":popover-open")) $("#panel").hidePopover();
  $("#panel").hidden = true;
  $("#panel").tether?.remove();
  sourceId = null;
  selected = null;
  renderConnections();
  [...document.querySelectorAll(".map-node")]
    .find((node) => node.dataset.id === origin)
    ?.focus({ preventScroll: true });
}

function appearanceFields(value, defaultIcon = "") {
  const icon = value.icon ?? defaultIcon;
  const colors = [...new Set([...palettes[palette], ...Object.values(palettes).map(colors => colors[3]), ...Object.values(palettes).flat()])].slice(0, 20);
  return '<details><summary>Colori</summary><input type="hidden" name="color" value="' + esc(value.color || "") + '"><div class="color-grid" role="group" aria-label="Colori delle palette">' +
    colors.map(color => '<button type="button" class="color-choice" data-color="' + color + '" aria-label="' + color + '" aria-pressed="' + (value.color === color) + '"></button>').join("") +
    '</div></details><details><summary>Icone</summary><input type="hidden" name="icon" value="' + esc(icon) + '"><div class="icon-grid" role="group" aria-label="Icone"><button type="button" data-icon="" title="Iniziale" aria-label="Iniziale" aria-pressed="' + (icon === "") + '">A</button>' +
    Object.entries(icons).map(([id, svg]) => '<button type="button" data-icon="' + id + '" title="' + iconNames[id] + '" aria-label="' + iconNames[id] + '" aria-pressed="' + (icon === id) + '"><svg viewBox="0 0 24 24" aria-hidden="true">' + svg + '</svg></button>').join("") +
    '</div></details><details><summary>Carica icona</summary><button type="button" class="icon-drop">Trascina o scegli un’immagine</button><input type="file" hidden class="icon-upload" accept="image/png,image/jpeg,image/webp,image/gif" aria-label="Carica o sostituisci icona"><div class="appearance-preview"></div></details>';
}

function bindAppearanceFields(panel) {
  const icon = panel.querySelector('[name="icon"]'), color = panel.querySelector('[name="color"]');
  let uploading = false;
  const refresh = () => {
    panel.querySelectorAll('[data-icon]').forEach(button => button.setAttribute("aria-pressed", String(button.dataset.icon === icon.value)));
    panel.querySelectorAll('[data-color]').forEach(button => {
      button.style.backgroundColor = button.dataset.color;
      button.setAttribute("aria-pressed", String(button.dataset.color === (color.value || automaticColor(panel.dataset.objectId))));
    });
    panel.querySelector('.appearance-preview').innerHTML = icon.value.startsWith("data:") ? iconBody(icon.value, "") : "";
  };
  panel.querySelectorAll('[data-icon]').forEach(button => button.onclick = () => { icon.value = button.dataset.icon; refresh(); });
  panel.querySelectorAll('[data-color]').forEach(button => button.onclick = () => { color.value = button.dataset.color; refresh(); });
  panel.querySelectorAll('details').forEach(section => {
    section.name = 'editor-' + panel.id;
    section.ontoggle = () => positionContext(panel, panel.opener);
  });
  const upload = async file => {
    if (!file) return;
    if (uploading) throw Error('Attendi il caricamento dell’icona.');
    if (!["image/png", "image/jpeg", "image/webp", "image/gif"].includes(file.type) || file.size > 5 * 1024 * 1024)
      throw Error("Scegli un’immagine PNG, JPEG, WebP o GIF fino a 5 MB.");
    uploading = true;
    try {
      const bitmap = await createImageBitmap(file), canvas = document.createElement("canvas");
      canvas.width = canvas.height = 64;
      const scale = Math.min(64 / bitmap.width, 64 / bitmap.height);
      canvas.getContext("2d").drawImage(bitmap, (64 - bitmap.width * scale) / 2, (64 - bitmap.height * scale) / 2, bitmap.width * scale, bitmap.height * scale);
      bitmap.close();
      icon.value = canvas.toDataURL("image/png"); refresh();
    } finally { uploading = false; }
  };
  const input = panel.querySelector('.icon-upload'), zone = panel.querySelector('.icon-drop');
  input.onchange = guard(event => upload(event.target.files[0]));
  zone.onclick = () => input.click();
  zone.ondragover = event => {
    if (!event.dataTransfer.types.includes('Files')) return;
    event.preventDefault(); event.stopPropagation(); zone.dataset.dropOver = "true";
  };
  zone.ondragleave = () => delete zone.dataset.dropOver;
  zone.ondrop = guard(async event => {
    event.preventDefault(); event.stopPropagation(); delete zone.dataset.dropOver;
    await upload(event.dataTransfer.files[0]);
  });
  refresh();
  return () => {
    if (uploading) throw Error("Attendi il caricamento dell’icona.");
    return { icon: icon.value, color: color.value };
  };
}

function scopeFields(scopes = {}, entry = {}) {
  return '<div class="scope-tools" role="group" aria-label="Cosa sincronizzare">' +
    [["chat", "chat", "Chat"], ["paths", "folder", "Percorsi"], ["apps", "box", "App, plugin e configurazioni"]].map(([scope, icon, label]) =>
      '<div class="scope-row"><button type="button" class="scope-toggle" data-scope="' + scope + '" aria-pressed="' + (scopes[scope] !== false) + '" aria-label="' + label + '" title="' + label + '"><svg viewBox="0 0 24 24" aria-hidden="true">' + icons[icon] + '</svg></button><div class="scope-detail" data-detail="' + scope + '" tabindex="0" aria-label="' + label + '"></div><button type="button" class="icon-button" data-refresh="' + scope + '" aria-label="Aggiorna ' + label + '" title="Aggiorna ' + label + '"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v5h-5M20 12a8 8 0 1 0-2 5"></path></svg></button></div>').join("") + '</div>';
}

function activityState(entry, origin, scope) {
  const key = origin + ":" + entry.id;
  const analysis = data.analyses?.[key] || entry.analysis || {};
  const job = data.job?.chat_id === entry.id ? data.job : data.transfers?.[key] || {};
  const transferring = job.chat_id === entry.id && job.source_device_id === origin &&
    (scope === "all" || job.scopes?.[scope] === true);
  const checking = (scope === "paths" || scope === "all") && analysis.status === "pending" || (scope === "apps" || scope === "all") && data.app_checks?.[key]?.status === "pending";
  const busy = !data.paused && transferring && job === data.job && job.status === "running" || checking;
  return { state: busy ? "busy" : "idle", label: checking ? "Check in corso" : "Sincronizzazione in corso" };
}

function activityIndicator(entry, origin, scope) {
  const value = activityState(entry, origin, scope);
  const indicator = document.createElement("span");
  indicator.className = "operation-indicator";
  indicator.dataset.state = value.state;
  indicator.hidden = value.state === "idle";
  indicator.title = value.label;
  indicator.setAttribute("role", "status");
  indicator.setAttribute("aria-label", value.label);
  return indicator;
}

function updateScopeText(box, lines) {
  const content = JSON.stringify(lines);
  if (box.dataset.content === content) return false;
  const selection = window.getSelection();
  if (selection?.rangeCount && !selection.isCollapsed && selection.getRangeAt(0).intersectsNode(box)) return false;
  box.replaceChildren(...lines.map(text => {
    const line = document.createElement("div"); line.textContent = text; return line;
  }));
  box.dataset.content = content;
  return true;
}

function appCheckLines(check) {
  const label = item => (item.kind === "skill" ? "Skill · " : item.kind === "plugin" ? "Plugin · " : "") + item.label + (item.version ? " " + item.version : "");
  if (!check || check.status === "missing") return ["Requisiti non ancora rilevati"];
  if (check.status === "waiting") return ["In attesa dei percorsi da analizzare"];
  if (check.status === "pending") return ["Rilevamento requisiti in corso…"];
  if (["failed", "unavailable"].includes(check.status)) return [check.error];
  const lines = (check.requirements || []).map(label);
  lines.push(...(check.configs || []).map(config => config.service + " · " + config.path));
  return lines.length ? lines : ["Nessuna app aggiuntiva individuata"];
}

function renderScopeDetails(panel, entry) {
  panel.scopeEntry = entry;
  const scopes = data.chat_settings?.[entry.id]?.scopes;
  if (scopes) panel.querySelectorAll('[data-scope]').forEach(button => {
    if (!button.disabled) button.setAttribute('aria-pressed', String(scopes[button.dataset.scope] !== false));
  });
  const origin = panel.dataset.sourceDevice || "master";
  const result = data.analyses?.[origin + ":" + entry.id] || entry.analysis || {}, manifest = result.manifest || {};
  const activity = entry.modified_ns ? new Date(Number(entry.modified_ns) / 1e6).toLocaleString("it-IT") : "Non disponibile";
  const chat = panel.querySelector('[data-detail="chat"]');
  if (!chat) return;
  let changed = updateScopeText(chat, [((entry.bytes || 0) / 1048576).toLocaleString("it-IT", { maximumFractionDigits: 2 }) + " MB · Ultima attività: " + activity]);
  const state = result.status === "failed" ? "Analisi non riuscita. " + (result.error || "") : result.status === "pending" ? "Analisi in corso…" : "Analisi da eseguire";
  const write = (scope, items, empty) => {
    const box = panel.querySelector('[data-detail="' + scope + '"]');
    const lines = result.status !== "ready" ? [state] : items.length ? items : [empty];
    changed = updateScopeText(box, lines) || changed;
  };
  const preparation = data.preparations?.[origin + ":" + entry.id];
  const paths = (manifest.paths || []).map(path => {
    const size = preparation?.paths?.find(item => item.path === path);
    return path + (Number.isFinite(size?.bytes) ? " · " + (size.bytes / 1048576).toLocaleString("it-IT", { maximumFractionDigits: 2 }) + " MB" : size?.error ? " · Dimensione non disponibile: " + size.error : "");
  });
  write("paths", paths, "Nessun percorso individuato");
  if (panel.manualRefresh?.apps && data.app_checks?.[origin + ":" + entry.id]?.status !== "pending") panel.manualRefresh.apps = false;
  changed = updateScopeText(panel.querySelector('[data-detail="apps"]'), panel.manualRefresh?.apps ? ["Rilevamento requisiti in corso…"] : appCheckLines(data.app_checks?.[origin + ":" + entry.id])) || changed;
  for (const scope of ["chat", "paths", "apps"]) {
    const row = panel.querySelector('[data-detail="' + scope + '"]').parentElement;
    row.querySelector(".operation-indicator")?.remove();
    const refresh = row.querySelector('[data-refresh]');
    const pending = scope === 'apps' ? data.app_checks?.[origin + ":" + entry.id]?.status === 'pending' : scope === 'paths' && result.status === 'pending';
    if (panel.manualRefresh?.[scope] && !pending) panel.manualRefresh[scope] = false;
    if (refresh) refresh.disabled = pending;
    const indicator = activityIndicator(entry, origin, scope);
    if (panel.manualRefresh?.[scope]) { indicator.hidden = false; indicator.dataset.state = 'busy'; }
    row.append(indicator);
  }
  if (changed) positionContext(panel, panel.opener);
}

async function saveChatScopes(entry, scopes, origin = sourceId || "master") {
  const result = await saveLinkChange(() => api("scopes", { source_device_id: origin, chat_id: entry.id, scopes }));
  data.chat_settings = result.preferences; chatLinks = result.links;
  if (link?.entry.id === entry.id) link.scopes = scopes;
  renderLink(true);
}

async function removePresentation(kind, id) {
  if (removingPresentation) return;
  const entry = visibleChats.find(chat => chat.id === id) || chatLinks.find(link => link.entry.id === id)?.entry || { id, name: id };
  const name = kind === "chat" ? chatLabel(entry) : deviceName(id);
  if (!window.confirm('Eliminare “' + name + '” da Oaclis e rimuovere tutti i suoi collegamenti?\n\n' +
      (kind === "chat" ? 'La chat verrà nascosta da Oaclis.\n' : 'Le impostazioni del dispositivo verranno rimosse.\n') +
      'Le chat Codex, i file trasferiti e le app resteranno intatti.')) return;
  removingPresentation = true;
  const matches = connection => kind === "chat" ? connection.entry.id === id : [connection.source, connection.target].includes(id);
  const removedLinks = chatLinks.filter(matches);
  const oldDevice = data.devices.find(device => device.id === id);
  const oldPreference = data.chat_settings?.[id];
  const oldLink = link;
  chatLinks = chatLinks.filter(connection => !matches(connection));
  if (link && matches(link)) link = null;
  if (kind === "chat") {
    data.chat_settings ??= {};
    data.chat_settings[id] = { ...oldPreference, hidden: true };
  } else data.devices = data.devices.filter(device => device.id !== id);
  // Remove existing elements directly so the browser can paint before the request finishes.
  document.querySelectorAll(".link-badge").forEach(marker => {
    const [chat, source, target] = JSON.parse(marker.dataset.linkKey);
    if (kind === "chat" ? chat === id : [source, target].includes(id)) disposeChatLink(marker.closest(".chat-link"));
  });
  if (kind === "device") document.querySelectorAll(".map-node").forEach(node => { if (node.dataset.id === id) node.remove(); });
  renderLibrary();
  try {
    const result = await saveLinkChange(() => api(kind === "chat" ? "chat/hide" : "device/delete", { id }));
    data.chat_settings = result.preferences;
    chatLinks = result.links;
  } catch (error) {
    for (const connection of removedLinks) {
      if (!chatLinks.some(current => current.entry.id === connection.entry.id && current.source === connection.source && current.target === connection.target)) chatLinks.push(connection);
    }
    if (kind === "chat") {
      if (oldPreference) data.chat_settings[id] = oldPreference;
      else delete data.chat_settings[id];
    } else if (oldDevice && !data.devices.some(device => device.id === id)) data.devices.push(oldDevice);
    link = oldLink;
    renderMap(); renderLibrary();
    throw error;
  } finally { removingPresentation = false; }
  document.querySelectorAll(".chat-editor").forEach(panel => {
    if (kind === "chat" ? panel.dataset.objectId === id : panel.dataset.sourceDevice === id) closeChatEditor(panel);
  });
  closeDeviceSettings(); closeConnectionPopover();
  if (link && (kind === "chat" ? link.entry.id === id : [link.source, link.target].includes(id))) link = null;
  document.querySelectorAll('.chat-terminal').forEach(panel => {
    if (kind === "chat" ? panel.dataset.chatId === id : panel.dataset.deviceId === id) closeChatTerminal(panel);
  });
  if (kind === "device") {
    delete positions[id]; savePositions();
    if (sourceId === id) closePanel();
  }
  if (kind === "device") delete data.appearance?.["device:" + id];
  renderMap(); renderLibrary();
}

async function removeConnection(connection, panel) {
  if (removingPresentation) return;
  if (!window.confirm('Eliminare questo collegamento?\n\nLa chat resterà nella lista. Le copie Codex, i file e le app resteranno intatti.')) return;
  removingPresentation = true;
  closeChatEditor(panel);
  const matches = item => item.entry.id === connection.entry.id && item.source === connection.source && item.target === connection.target;
  const removed = chatLinks.filter(matches);
  chatLinks = chatLinks.filter(item => !matches(item));
  renderLink(true);
  try {
    await saveLinkChange(() => api("links/delete", connection));
  } catch (error) {
    for (const item of removed) if (!chatLinks.some(matches)) chatLinks.push(item);
    renderLink(true);
    throw new Error("Eliminazione non salvata. Collegamento ripristinato: " + error.message);
  } finally { removingPresentation = false; }
}

function positionContext(popover, anchor) {
  popover.classList.add("context-popover");
  const opener = anchor || document.activeElement || $("#add-device");
  if (opener.isConnected) {
    popover.anchorRect = opener.getBoundingClientRect();
    popover.anchorToolbar = opener.closest(".toolbar");
  }
  const origin = popover.anchorRect;
  if (!origin) return;
  const toolbar = popover.anchorToolbar;
  const top = toolbar ? toolbar.getBoundingClientRect().bottom + 8 : origin.top;
  popover.style.maxHeight = "calc(100dvh - " + (toolbar ? top + 8 : 16) + "px)";
  const rect = popover.getBoundingClientRect();
  if (popover.dataset.moved === "true") {
    popover.style.left = Math.max(8, Math.min(rect.left, innerWidth - rect.width - 8)) + "px";
    popover.style.top = Math.max(8, Math.min(rect.top, innerHeight - rect.height - 8)) + "px";
    return;
  }
  popover.style.left = Math.max(8, Math.min(toolbar ? origin.left : origin.right + 8, innerWidth - rect.width - 8)) + "px";
  popover.style.top = Math.max(8, Math.min(top, innerHeight - rect.height - 8)) + "px";
}

function bindPanelDrag(panel, reference = false) {
  const heading = panel.querySelector("h2, .panel-bar");
  panel.classList.add("draggable-panel");
  if (panel.matches(".chat-terminal, #orchestrator-log")) {
    panel.classList.add("resizable-panel");
    panel.addEventListener("pointerdown", () => {
      const rect = panel.getBoundingClientRect();
      const width = Math.max(0, innerWidth - rect.left - 8), height = Math.max(0, innerHeight - rect.top - 8);
      panel.style.minWidth = Math.min(280, width) + "px";
      panel.style.minHeight = Math.min(240, height) + "px";
      panel.style.maxWidth = width + "px";
      panel.style.maxHeight = height + "px";
    });
  }
  const move = (left, top) => {
    if (panel.id === "panel" && panel.dataset.moved !== "true") {
      panel.style.width = panel.offsetWidth + "px"; panel.style.right = "auto";
    }
    panel.dataset.moved = "true";
    panel.style.left = Math.max(8, Math.min(left, innerWidth - panel.offsetWidth - 8)) + "px";
    panel.style.top = Math.max(8, Math.min(top, innerHeight - panel.offsetHeight - 8)) + "px";
  };
  heading.tabIndex = 0;
  heading.onkeydown = event => {
    const directions = { ArrowLeft: [-10, 0], ArrowRight: [10, 0], ArrowUp: [0, -10], ArrowDown: [0, 10] };
    if (!directions[event.key] || event.target !== heading) return;
    event.preventDefault();
    const [x, y] = directions[event.key];
    move(panel.offsetLeft + x, panel.offsetTop + y);
  };
  heading.onpointerdown = event => {
    if (event.button !== 0 || event.target.closest("button, input, textarea, select")) return;
    event.preventDefault();
    if (panel.hasAttribute("popover")) { panel.hidePopover(); panel.showPopover(); }
    const start = { x: event.clientX, y: event.clientY, left: panel.offsetLeft, top: panel.offsetTop };
    panel.dataset.dragging = "true";
    heading.setPointerCapture(event.pointerId);
    heading.onpointermove = e => {
      if (heading.hasPointerCapture(e.pointerId)) move(start.left + e.clientX - start.x, start.top + e.clientY - start.y);
    };
    const finish = e => {
      delete panel.dataset.dragging;
      if (heading.hasPointerCapture(e.pointerId)) heading.releasePointerCapture(e.pointerId);
    };
    heading.onpointerup = finish;
    heading.onpointercancel = finish;
    heading.onlostpointercapture = () => delete panel.dataset.dragging;
  };
  if (reference) bindPanelReference(panel);
}

function bindPanelReference(panel) {
  panel.tether?.remove();
  const original = panel.opener.closest(".map-node, .link-badge, .library-row") || panel.opener;
  const nodeId = original.dataset.id, linkKey = original.dataset.linkKey;
  const chatId = original.querySelector("[data-chat-id]")?.dataset.chatId, chatSource = original.dataset.sourceDevice;
  const resolve = () => nodeId ? [...document.querySelectorAll(".map-node")].find(n => n.dataset.id === nodeId)
    : linkKey ? [...document.querySelectorAll(".link-badge")].find(n => n.dataset.linkKey === linkKey)
    : chatId ? [...document.querySelectorAll(".library-item")].find(n => n.dataset.chatId === chatId && n.closest(".library-row").dataset.sourceDevice === chatSource)?.closest(".library-row") : original;
  const tether = document.createElement("div");
  tether.className = "panel-tether";
  tether.setAttribute("popover", "manual"); tether.setAttribute("aria-hidden", "true");
  tether.innerHTML = '<svg><line/></svg>';
  panel.tether = tether;
  document.body.append(tether); tether.showPopover();
  if (panel.hasAttribute("popover")) { panel.hidePopover(); panel.showPopover(); }
  const line = tether.querySelector("line");
  const edge = (rect, target) => {
    const x = rect.left + rect.width / 2, y = rect.top + rect.height / 2;
    const dx = target.x - x, dy = target.y - y;
    const scale = Math.min(dx ? rect.width / 2 / Math.abs(dx) : Infinity, dy ? rect.height / 2 / Math.abs(dy) : Infinity);
    const ratio = Number.isFinite(scale) ? Math.min(1, scale) : 0;
    return { x: x + dx * ratio, y: y + dy * ratio };
  };
  const update = () => {
    if (!panel.isConnected || panel.hidden || panel.tether !== tether) { tether.remove(); return; }
    const object = resolve();
    tether.style.visibility = object?.isConnected ? "visible" : "hidden";
    if (object?.isConnected) {
      const a = object.getBoundingClientRect(), b = panel.getBoundingClientRect();
      const heading = panel.querySelector("h2, .panel-bar").getBoundingClientRect();
      const end = { x: a.left + a.width / 2 <= b.left + b.width / 2 ? b.left : b.right,
        y: heading.top + heading.height / 2 };
      const start = edge(a, end);
      for (const [name, value] of Object.entries({ x1: start.x, y1: start.y, x2: end.x, y2: end.y })) line.setAttribute(name, value);
    }
    requestAnimationFrame(update);
  };
  requestAnimationFrame(update);
}

function closeDeviceSettings() {
  const popover = $("#device-settings");
  if (!popover) return;
  const anchor = popover.opener;
  popover.remove();
  if (anchor?.isConnected) anchor.focus({ preventScroll: true });
}

function closeConnectionPopover() {
  const popover = $("#connection-popover");
  if (!popover) return;
  const details = $("#details");
  details.hidden = true;
  $("#panel").append(details);
  const anchor = popover.opener;
  popover.remove();
  selected = null;
  if (anchor?.isConnected) anchor.focus({ preventScroll: true });
}

function closeChatEditor(popover = document.activeElement?.closest(".chat-editor") || [...document.querySelectorAll(".chat-editor")].at(-1)) {
  if (!popover) return;
  const anchor = popover.opener;
  clearInterval(popover.detailsTimer);
  popover.remove();
  if (anchor?.isConnected) anchor.focus({ preventScroll: true });
}

function editChat(entry, anchor, origin = sourceId || "master", connection = null) {
  const connectionKey = connection ? JSON.stringify([connection.source, connection.target]) : "";
  const existing = [...document.querySelectorAll(".chat-editor")].find(panel => panel.dataset.objectId === entry.id && panel.dataset.sourceDevice === origin && panel.dataset.connectionKey === connectionKey);
  if (existing) {
    existing.opener = anchor; bindPanelReference(existing);
    existing.querySelector('[name="label"]').focus();
    return existing;
  }
  const value = data.appearance?.["chat:" + entry.id] || {};
  const scopes = data.chat_settings?.[entry.id]?.scopes || chatLinks.find(v => v.entry.id === entry.id)?.scopes || {};
  const panel = document.createElement("div");
  panel.id = "chat-editor-" + crypto.randomUUID(); panel.classList.add("chat-editor"); panel.opener = anchor; panel.dataset.objectId = entry.id; panel.dataset.sourceDevice = origin; panel.dataset.connectionKey = connectionKey;
  panel.setAttribute("popover", "manual"); panel.setAttribute("role", "dialog"); panel.setAttribute("aria-label", "Personalizza chat");
  panel.innerHTML = '<button type="button" class="icon-button chat-visibility" aria-label="Nascondi chat"></button><button type="button" class="icon-button popover-close" aria-label="Chiudi modifica chat"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg></button><h2>' + esc(chatLabel(entry)) + '</h2><form><label class="field">Nome visualizzato<input name="label" maxlength="160" value="' + esc(value.label || "") + '" placeholder="' + esc(chatLabel(entry)) + '"></label>' + appearanceFields(value) + '<details><summary>Sincronizza</summary>' + scopeFields(scopes, entry) + '</details><button class="primary">Salva</button><button type="button" class="delete-presentation">Elimina</button></form>';
  document.body.append(panel); panel.showPopover(); positionContext(panel, anchor);
  bindPanelDrag(panel, true);
  panel.querySelector('.popover-close').onclick = () => closeChatEditor(panel);
  const visibility = panel.querySelector('.chat-visibility');
  const updateVisibility = () => {
    const hidden = data.chat_settings?.[entry.id]?.hidden === true;
    renderEye(visibility, hidden, hidden ? "Mostra chat" : "Nascondi chat");
  };
  updateVisibility();
  visibility.onclick = guard(async () => {
    visibility.disabled = true;
    try {
      const result = await api("chat/visibility", { id: entry.id, hidden: data.chat_settings?.[entry.id]?.hidden !== true });
      data.chat_settings = result.preferences;
      updateVisibility(); renderLibrary();
    } finally { visibility.disabled = false; }
  });
  panel.updateVisibility = updateVisibility;
  renderScopeDetails(panel, entry);
  panel.detailsTimer = setInterval(async () => {
    try {
      const catalog = await api("catalog", { device_id: origin });
      const fresh = catalog.chats.find(c => c.id === entry.id);
      if (fresh && panel.isConnected) renderScopeDetails(panel, fresh);
    } catch (_) { /* Existing details remain available while the device is offline. */ }
  }, 10000);
  panel.querySelectorAll('[data-refresh]').forEach(button => button.onclick = guard(async () => {
    const scope = button.dataset.refresh;
    button.disabled = true;
    try {
      if (scope === 'chat') {
        const result = await api("catalog", { device_id: origin });
        const fresh = result.chats.find(chat => chat.id === entry.id);
        if (!fresh) throw new Error("Chat non trovata sul dispositivo");
        renderScopeDetails(panel, fresh);
      } else {
        const result = await api(scope === 'apps' ? "app-check" : "analyze", { source_device_id: origin, chat_id: entry.id, force: true });
        const collection = scope === 'apps' ? 'app_checks' : 'analyses';
        data[collection] ??= {}; data[collection][origin + ":" + entry.id] = result;
        panel.manualRefresh ??= {}; panel.manualRefresh[scope] = result.status === 'pending';
        renderScopeDetails(panel, panel.scopeEntry);
      }
    } finally { if (!panel.manualRefresh?.[scope]) button.disabled = false; }
  }));
  const readAppearance = bindAppearanceFields(panel);
  const save = async value => {
    const key = "chat:" + entry.id;
    await api("appearance", { key, value }); data.appearance ??= {}; data.appearance[key] = value;
    closeChatEditor(panel); renderLibrary(); renderLink(true);
  };
  panel.querySelector('form').onsubmit = guard(async event => { event.preventDefault(); await save({ label: event.target.elements.label.value, ...readAppearance() }); });
  panel.querySelector('.delete-presentation').onclick = guard(() => connection ? removeConnection(connection, panel) : removePresentation("chat", entry.id));
  panel.querySelectorAll('[data-scope]').forEach(button => button.onclick = guard(async () => {
    const previous = button.getAttribute('aria-pressed'); button.setAttribute('aria-pressed', String(previous !== 'true'));
    panel.querySelectorAll('[data-scope]').forEach(b => b.disabled = true);
    try { await saveChatScopes(entry, Object.fromEntries([...panel.querySelectorAll('[data-scope]')].map(b => [b.dataset.scope, b.getAttribute('aria-pressed') === 'true'])), origin); }
    catch (error) { button.setAttribute('aria-pressed', previous); throw error; }
    finally { panel.querySelectorAll('[data-scope]').forEach(b => b.disabled = false); }
  }));
  panel.querySelector('[name="label"]').focus();
  return panel;
}

function focusDetails() {
  const heading = $("#details h2");
  heading.tabIndex = -1;
  heading.focus({ preventScroll: true });
}

function field(label, name, value = "") {
  return (
    '<label class="field">' +
    label +
    '<input name="' +
    name +
    '" value="' +
    esc(value) +
    '" required></label>'
  );
}

function editDevice(device = {}, anchor = $("#add-device")) {
  const settings = data.ssh_settings?.[device.id] || {}, value = data.appearance?.["device:" + device.id] || {};
  const local = device.id === "master";
  const objectId = device.id || "device-" + crypto.randomUUID();
  closeDeviceSettings();
  const panel = document.createElement("div"); panel.id = "device-settings"; panel.opener = anchor; panel.dataset.objectId = objectId;
  panel.setAttribute("popover", "manual"); panel.setAttribute("role", "dialog"); panel.setAttribute("aria-label", device.id ? "Modifica dispositivo" : "Aggiungi dispositivo");
  panel.innerHTML = '<button type="button" class="icon-button popover-close" aria-label="Chiudi dispositivo"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg></button><h2>' + (device.id ? "Modifica dispositivo" : "Aggiungi dispositivo") + '</h2><form id="device-form" novalidate>' + field("Nome", "name", device.name || "") + appearanceFields(value, "desktop") +
    (local ? '' : '<details class="device-advanced"><summary>Avanzato</summary>' + field("Indirizzo IP o hostname", "address", settings.address || "") + '<label class="field">Porta SSH<input type="number" name="port" min="1" max="65535" required value="' + esc(settings.port || 22) + '"></label>' + field("Utente SSH", "user", settings.user || "") + '<label class="field">Chiave SSH (facoltativa)<input name="identity_file" value="' + esc(settings.identity_file || "") + '" placeholder="~/.ssh/id_ed25519"></label></details>') + '<button class="primary" type="submit">Salva</button>' + (device.id && !local ? '<button type="button" class="delete-presentation">Elimina</button>' : '') + '</form>';
  document.body.append(panel); panel.showPopover(); positionContext(panel, anchor);
  bindPanelDrag(panel, true);
  panel.querySelector('.popover-close').onclick = closeDeviceSettings;
  const readAppearance = bindAppearanceFields(panel);
  panel.querySelector('.delete-presentation')?.addEventListener('click', guard(() => removePresentation("device", device.id)));
  panel.querySelector('form').onsubmit = guard(async event => {
    event.preventDefault(); const form = event.target;
    if (!local && ![form.elements.address, form.elements.port, form.elements.user].every(e => e.checkValidity())) panel.querySelector('.device-advanced').open = true;
    if (!form.reportValidity()) return;
    const appearanceValue = { label: local ? form.elements.name.value : value.label || "", ...readAppearance() };
    if (local) {
      await api("appearance", { key: "device:master", value: appearanceValue });
      data.appearance ??= {}; data.appearance['device:master'] = appearanceValue; master.name = appearanceValue.label;
    } else {
      await api("device", {
        id: objectId, name: form.elements.name.value,
        ...(device.ssh_alias ? { ssh_alias: device.ssh_alias } : {}), appearance: appearanceValue,
        ssh_settings: { address: form.elements.address.value.trim(), port: Number(form.elements.port.value), user: form.elements.user.value.trim(), identity_file: form.elements.identity_file.value.trim() },
      });
      data = await api("state"); chatLinks = data.links || [];
    }
    closeDeviceSettings(); renderMap();
    if (sourceId === device.id) $("#panel-device").textContent = form.elements.name.value;
    await api("check", {});
  });
  panel.querySelector('[name="name"]').focus();
}

function openDiscovery() {
  closeDeviceSettings();
  let panel = $("#discovery-popover");
  if (!panel) {
    panel = document.createElement("div"); panel.id = "discovery-popover"; panel.opener = $("#add-device");
    panel.setAttribute("popover", "manual"); panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-label", "Collega dispositivo");
    panel.innerHTML = '<button class="icon-button popover-close" type="button" aria-label="Chiudi"><svg viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18"/></svg></button><h2>Collega dispositivo</h2><div class="discovery-content" aria-live="polite"></div><button class="primary discovery-manual" type="button"><svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 21h8m-4-5v5"/></svg><span>Aggiungi manualmente</span></button><button class="primary discovery-manual discovery-mobile" type="button"><svg viewBox="0 0 24 24" aria-hidden="true"><rect x="6" y="2" width="12" height="20" rx="2"/><path d="M10 18h4"/></svg><span>Aggiungi mobile</span></button>';
    document.body.append(panel); panel.showPopover();
    panel.querySelector('.popover-close').onclick = () => panel.remove();
    panel.querySelector('.discovery-manual').onclick = () => { panel.remove(); editDevice(); };
    panel.querySelector('.discovery-mobile').hidden = data.role === 'client';
    panel.querySelector('.discovery-mobile').onclick = () => { panel.remove(); openMobileInvite(); };
  }
  renderDiscovery(); positionContext(panel, $("#add-device"));
}

function renderDiscovery() {
  const panel = $("#discovery-popover"); if (!panel) return;
  panel.querySelector('.discovery-mobile').hidden = data.role === 'client';
  const content = panel.querySelector('.discovery-content');
  const html = (discovery.setup_needed || discovery.network?.status === 'pending' ? '<button type="button" class="discovery-network" ' + (discovery.network?.status === 'pending' ? 'disabled' : '') + '>Prepara collegamento</button><small>' + (discovery.network?.status === 'pending' ? 'Attendi la conferma di Windows…' : 'La rete virtuale richiede un inoltro. Windows chiederà la conferma amministrativa.') + '</small>' : '') +
    (discovery.network?.error ? '<p class="discovery-error">' + esc(discovery.network.error) + '</p>' : '') +
    (discovery.error ? '<p class="discovery-error">' + esc(discovery.error) + '</p>' : '') +
    (discovery.ignored ? '<button type="button" class="discovery-restore">Mostra dispositivi rifiutati</button>' : '') +
    (discovery.peers.length ? discovery.peers.map(peer => '<section class="discovery-peer"><strong>' + esc(peer.name) + '</strong><small>' + esc(peer.address) + '</small>' +
      (peer.connected ? '<p>Collegamento autorizzato</p>' : '<p>Confronta questo codice sui due dispositivi.</p><strong class="pairing-code">' + esc(peer.code) + '</strong>' +
        (peer.accepted ? '<p>' + (peer.remote_accepted ? 'Autorizzato. Preparazione SSH in corso.' : 'In attesa della conferma sull’altro dispositivo.') + '</p>' : '<button type="button" class="primary discovery-accept" data-peer="' + esc(peer.id) + '">Il codice coincide · Accetta</button>') +
        (peer.ssh_ready === false ? '<small>SSH da attivare sul dispositivo.</small>' : '') +
        '<button type="button" class="discovery-reject" data-peer="' + esc(peer.id) + '">Rifiuta</button>') + '</section>').join('') : '<p>Ricerca dispositivi…</p>');
  if (content.dataset.view === html) return;
  content.innerHTML = html; content.dataset.view = html;
  content.querySelector('.discovery-network')?.addEventListener('click', guard(async () => {
    const result = await api('peers/network', {}); notice(result.message);
  }));
  content.querySelector('.discovery-restore')?.addEventListener('click', guard(async () => {
    await api('peers/restore', {}); await updateDiscovery(await api('peers'));
  }));
  for (const button of content.querySelectorAll('[data-peer]')) button.onclick = guard(async () => {
    button.disabled = true;
    try { await api('peers/approve', { id: button.dataset.peer, accept: button.classList.contains('discovery-accept') }); await updateDiscovery(await api('peers')); }
    finally { if (button.isConnected) button.disabled = false; }
  });
  positionContext(panel, $("#add-device"));
}

async function updateDiscovery(next, first = false) {
  discovery = next;
  const fresh = next.peers.filter(peer => !noticedPeers.has(peer.id));
  for (const peer of fresh) noticedPeers.add(peer.id);
  const newRegistration = next.peers.some(peer => peer.device_id && !data.devices.some(device => device.id === peer.device_id));
  if (newRegistration && !removingPresentation) { data = await api('state'); chatLinks = data.links || []; renderMap(); await api('check', {}); }
  if (fresh.some(peer => !peer.connected)) { notice('Oaclis trovato nella rete'); beep(); }
  if ((first && !data.devices.length) || fresh.some(peer => !peer.accepted && !peer.connected)) {
    if (!document.querySelector('.context-popover:popover-open')) openDiscovery();
  }
  renderDiscovery();
}

let completedPathAuthorization = null;
function renderPathAuthorization(auth) {
  let panel = document.querySelector('#path-authorization');
  if (!auth || auth.id === completedPathAuthorization) {
    if (panel) { panel.querySelector('input').value = ''; panel.close(); panel.remove(); }
    return;
  }
  if (panel?.dataset.request === auth.id) return;
  if (panel) { panel.close(); panel.remove(); }
  panel = document.createElement('dialog');
  panel.id = 'path-authorization'; panel.className = 'context-popover path-authorization'; panel.dataset.request = auth.id;
  panel.setAttribute('aria-labelledby', 'path-authorization-title');
  panel.innerHTML = '<button type="button" class="icon-button popover-close" aria-label="Annulla autorizzazione"><svg viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18"/></svg></button>' +
    '<h2 id="path-authorization-title">Prepara cartella</h2><p class="authorization-device"></p><p class="authorization-path"></p>' +
    '<p>Creare il percorso e assegnare la sola cartella finale all’utente indicato. Le cartelle esistenti mantengono proprietario e permessi.</p>' +
    '<form autocomplete="off"><label>Password sudo<input name="password" type="password" required autocomplete="off" maxlength="1024" aria-describedby="path-authorization-note"></label>' +
    '<small id="path-authorization-note">Usata solo per questa operazione, senza salvarla.</small><p class="authorization-error" role="alert"></p>' +
    '<button class="primary" type="submit">Autorizza e continua</button><button class="authorization-cancel" type="button">Annulla</button></form>';
  panel.querySelector('.authorization-device').textContent = auth.device + (auth.user ? ' · ' + auth.user : '');
  panel.querySelector('.authorization-path').textContent = auth.path;
  let busy = false;
  const submit = async cancel => {
    if (busy) return;
    busy = true;
    const field = panel.querySelector('input');
    let password = field.value; field.value = '';
    panel.querySelector('.authorization-error').textContent = '';
    panel.querySelectorAll('button,input').forEach(node => node.disabled = true);
    panel.querySelector('[type="submit"]').textContent = cancel ? 'Annullamento…' : 'Autorizzazione…';
    try {
      const pending = api('path/authorize', cancel ? {id:auth.id,cancel:true} : {id:auth.id,password});
      password = '';
      await pending;
      completedPathAuthorization = auth.id; panel.close(); panel.remove();
    } catch (error) {
      panel.querySelector('.authorization-error').textContent = error.message;
    } finally {
      password = ''; busy = false;
      if (panel.isConnected) {
        panel.querySelectorAll('button,input').forEach(node => node.disabled = false);
        panel.querySelector('[type="submit"]').textContent = 'Autorizza e continua'; field.focus();
      }
    }
  };
  panel.querySelector('form').onsubmit = event => {event.preventDefault(); submit(false);};
  panel.querySelector('.authorization-cancel').onclick = panel.querySelector('.popover-close').onclick = () => submit(true);
  panel.addEventListener('cancel', event => {event.preventDefault(); submit(true);});
  document.body.append(panel); panel.showModal(); panel.querySelector('input').focus();
}


const logPanel = document.querySelector("#orchestrator-log");
let logPolling = false;
bindPanelDrag(logPanel);
async function refreshOrchestratorLog() {
  if (logPanel.hidden || logPolling) return;
  logPolling = true;
  try {
    const rows = await api("log");
    const pre = logPanel.querySelector("pre");
    const atBottom = pre.scrollHeight - pre.scrollTop - pre.clientHeight < 24;
    const text = rows.map(row => new Date(row.time).toLocaleTimeString() + " " + row.text).join("\n");
    if (pre.textContent !== text) { pre.textContent = text; if (atBottom) pre.scrollTop = pre.scrollHeight; }
  } catch (error) { console.error(error); }
  finally { logPolling = false; }
}
document.querySelector("#toggle-log").onclick = () => {
  logPanel.hidden = !logPanel.hidden;
  document.querySelector("#toggle-log").setAttribute("aria-expanded", String(!logPanel.hidden));
  refreshOrchestratorLog();
};
logPanel.querySelector(".popover-close").onclick = () => {
  logPanel.hidden = true;
  document.querySelector("#toggle-log").setAttribute("aria-expanded", "false");
};
setInterval(refreshOrchestratorLog, 1000);


function renderMobileApproval(panel, device) {
  const content = panel.querySelector('.mobile-invite-content');
  content.innerHTML = '<h3>' + esc(device.name) + '</h3><p>Abbinamento completato. Autorizzare questo telefono?</p><button class="primary" type="button" data-accept="true">Autorizza</button><button class="delete-presentation" type="button" data-accept="false">Rifiuta</button><p class="mobile-status" role="status"></p>';
  positionContext(panel, panel.opener);
  for (const button of content.querySelectorAll('[data-accept]')) button.onclick = async () => {
    const accept = button.dataset.accept === 'true';
    content.querySelectorAll('button').forEach(b => b.disabled = true);
    content.querySelector('.mobile-status').textContent = accept ? 'Autorizzazione…' : 'Rifiuto…';
    try {
      const result = await api('mobile/approve', { id: device.id, accept });
      if (Array.isArray(result.devices)) { data.mobile_devices = result.devices; renderMap(true); }
      if (panel.isConnected) content.innerHTML = '<h3>' + esc(device.name) + '</h3><p>' + (accept ? 'Telefono autorizzato.' : 'Richiesta rifiutata.') + '</p>';
    } catch (_) {
      if (panel.isConnected) {
        content.querySelector('.mobile-status').textContent = 'Conferma non riuscita. Riprova.';
        content.querySelectorAll('button').forEach(b => b.disabled = false);
      }
    }
  };
}

async function openMobileInvite() {
  document.querySelector('#mobile-invite .popover-close')?.click();
  const panel = document.createElement('div');
  panel.id = 'mobile-invite'; panel.opener = $('#add-device');
  panel.setAttribute('popover', 'manual'); panel.setAttribute('role', 'dialog');
  panel.setAttribute('aria-label', 'Aggiungi mobile');
  panel.innerHTML = '<button class="icon-button popover-close" type="button" aria-label="Chiudi"><svg viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18"/></svg></button><h2>Aggiungi mobile</h2><div class="mobile-invite-content" aria-live="polite"><p>Preparazione…</p></div>';
  document.body.append(panel); panel.showPopover(); positionContext(panel, panel.opener); bindPanelDrag(panel);
  let expiry, timer, expiresAt;
  panel.querySelector('.popover-close').onclick = () => { clearTimeout(expiry); clearTimeout(timer); panel.remove(); };
  const content = panel.querySelector('.mobile-invite-content');
  const poll = async () => {
    if (!panel.isConnected) return;
    try {
      const pending = await api('mobile/pending');
      if (!panel.isConnected) return;
      if (pending.devices.length) { clearTimeout(expiry); renderMobileApproval(panel, pending.devices[0]); return; }
      content.querySelector('.mobile-status').textContent = '';
    } catch (_) {
      if (!panel.isConnected) return;
      const status = content.querySelector('.mobile-status');
      if (status) status.textContent = 'Collegamento momentaneamente non disponibile.';
    }
    if (panel.isConnected && Date.now() < expiresAt) timer = setTimeout(poll, 3000);
  };
  try {
    const pending = await api('mobile/pending');
    if (!panel.isConnected) return;
    if (pending.devices.length) { renderMobileApproval(panel, pending.devices[0]); return; }
    const result = await api('mobile/invite', {});
    if (!panel.isConnected) return;
    content.innerHTML = '<div class="mobile-qr" role="img" aria-label="QR di abbinamento telefono">' + result.qr_svg + '</div><p>Inquadra con il telefono.</p><small>Valido 10 minuti · un solo abbinamento</small><p class="mobile-status" role="status"></p>';
    positionContext(panel, panel.opener);
    expiresAt = Date.now() + result.expires_in * 1000;
    expiry = setTimeout(() => { clearTimeout(timer); if (panel.isConnected) content.innerHTML = '<p>QR scaduto. Chiudi e riapri per generarne uno nuovo.</p>'; }, result.expires_in * 1000);
    timer = setTimeout(poll, 3000);
  } catch (_) {
    if (panel.isConnected) content.innerHTML = '<p>Collegamento non disponibile. Chiudi e riprova.</p>';
  }
}


async function removeMobile(device) {
  if (removingPresentation || !window.confirm('Eliminare “' + device.name + '”?\n\nL’accesso del telefono verrà revocato e sarà rimosso dalla mappa. Per accedere nuovamente servirà un nuovo abbinamento.')) return;
  removingPresentation = true;
  document.querySelectorAll('.mobile-chat-editor').forEach(panel => { if (panel.dataset.objectId === device.id) panel.remove(); });
  const previous = data.mobile_devices;
  data.mobile_devices = (previous || []).filter(d => d.id !== device.id);
  renderMap();
  try {
    const result = await api('mobile/delete', { id: device.id });
    data.mobile_devices = result.devices;
    delete positions[device.id]; savePositions();
  } catch (_) {
    data.mobile_devices = previous;
    notice('Eliminazione non riuscita. Riprova.');
  } finally { removingPresentation = false; renderMap(); }
}


async function openMobileChats(device, anchor) {
  const existing = [...document.querySelectorAll('.mobile-chat-editor')].find(p => p.dataset.objectId === device.id);
  if (existing) { existing.querySelector('input')?.focus(); return; }
  const panel = document.createElement('div');
  panel.className = 'mobile-chat-editor'; panel.dataset.objectId = device.id; panel.opener = anchor;
  panel.setAttribute('popover', 'manual'); panel.setAttribute('role', 'dialog'); panel.setAttribute('aria-label', 'Chat per ' + device.name);
  panel.innerHTML = '<button type="button" class="icon-button popover-close" aria-label="Chiudi"><svg viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18"/></svg></button><h2>Chat autorizzate</h2><p>' + esc(device.name) + '</p><div class="mobile-chat-content"><p>Caricamento chat…</p></div>';
  document.body.append(panel); panel.showPopover(); positionContext(panel, anchor); bindPanelDrag(panel, true);
  panel.querySelector('.popover-close').onclick = () => panel.remove();
  panel.querySelector('h2').focus({preventScroll:true});
  const content = panel.querySelector('.mobile-chat-content');
  try {
    const result = await api('mobile/chats', {id:device.id});
    if (!panel.isConnected) return;
    const selected = new Set(result.selected || []);
    const chats = [...result.chats].sort((a,b) => a.name.localeCompare(b.name));
    content.innerHTML = '<form><label class="search"><input type="search" aria-label="Cerca chat" placeholder="Cerca chat"></label><div class="mobile-chat-options">' + (chats.length ? chats.map(chat => '<label class="mobile-chat-option"><input type="checkbox" value="' + esc(chat.id) + '" ' + (selected.has(chat.id) ? 'checked' : '') + '><span>' + esc(chat.name) + '</span></label>').join('') : '<p>Nessuna chat disponibile.</p>') + '</div><p class="mobile-chat-status" role="status"></p><button type="submit" class="primary">Salva</button></form>';
    const search = content.querySelector('input[type="search"]');
    search.oninput = () => { const term = search.value.trim().toLocaleLowerCase(); content.querySelectorAll('.mobile-chat-option').forEach(row => row.hidden = !row.textContent.toLocaleLowerCase().includes(term)); };
    content.querySelector('form').onsubmit = async event => {
      event.preventDefault(); const button = content.querySelector('button[type="submit"]'); button.disabled = true;
      try {
        await api('mobile/chats', {id:device.id,chats:[...content.querySelectorAll('input[type="checkbox"]:checked')].map(input=>input.value)});
        panel.remove(); notice('Chat autorizzate aggiornate');
      } catch (_) { if (panel.isConnected) content.querySelector('.mobile-chat-status').textContent = 'Salvataggio non riuscito. Riprova.'; }
      finally { button.disabled = false; }
    };
    positionContext(panel, anchor);
    if (panel.contains(document.activeElement)) search.focus({preventScroll:true});
  } catch (_) { if (panel.isConnected) content.innerHTML = '<p>Chat non disponibili. Chiudi e riprova.</p>'; }
}
