// map.js v3.121.2924
"use strict";
// Shared controller state and API helpers are defined in app.js.

function disposeChatLink(layer) {
  if (!layer) return;
  layer.querySelector('.link-badge')?.badgeMotion?.destroy();
  layer.remove();
}

function updateBadgeMotion(marker, connection) {
  const key = connection.source + ':' + connection.entry.id;
  const pair = [connection.entry.id, ...[connection.source, connection.target].sort()].join(':');
  const active = data.job?.link_key === pair;
  const current = active || data.job?.chat_id === connection.entry.id && data.job?.source_device_id === connection.source ? data.job : data.transfers?.[key] || {};
  const matches = (active || !current.device_id || current.device_id === connection.target) && (!current.generation || !connection.flow?.generation || current.generation === connection.flow.generation);
  const starting = marker.playPending || connection.flow?.requested === true;
  const modules = starting ? Object.fromEntries(["chat", "paths", "apps"].map(name => [name, { changed: connection.scopes?.[name] !== false && connection.flow?.modules?.[name]?.changed !== false, progress: 0 }])) : matches ? current.module_progress || {} : {};
  const changed = Object.values(modules).some(module => module.changed === true);
  const checking = connection.flow?.phase === 'checking' || data.app_checks?.[key]?.status === 'pending' || !(data.checks_paused ?? data.paused) && data.analyses?.[key]?.status === 'pending';
  const unfinished = Object.values(modules).some(module => module.changed === true && module.progress < 1);
  const remote = connection.source === 'master' ? connection.target : connection.source;
  const paused = changed && unfinished && (data.paused || current.status === 'paused' || current.status === 'failed' && connection.flow?.phase === 'failed' || data.connections[remote] === 'offline');
  const phase = starting ? 'transferring' : paused ? 'paused' : connection.flow?.phase === 'transferring' || matches && current.status === 'running' ? 'transferring' : checking ? 'checking' : 'idle';
  marker.badgeMotion.update({ phase, modules });
}

// Preview direction only: the original paths and transfer controls stay untouched.
function previewLinkFlow(node) {
  document.querySelectorAll('.link-flow-overlay').forEach(overlay => overlay.remove());
  if (!node || node.dataset.dragging || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
  const id = node.dataset.id;
  const paths = [...document.querySelectorAll('.chat-link')]
    .filter(layer => layer.dataset.source === id || layer.dataset.target === id)
    .map(layer => ({ path: layer.querySelector('.connection-lines > path'), both: layer.dataset.operation === 'sync' }));
  for (const path of document.querySelectorAll('#mobile-links > path')) {
    if (id === 'master' || path.dataset.id === id) paths.push({ path, both: false });
  }
  for (const { path, both } of paths) {
    const overlay = document.createElementNS(path.namespaceURI, 'g');
    overlay.classList.add('link-flow-overlay');
    for (const reverse of both ? [false, true] : [false]) {
      const trace = document.createElementNS(path.namespaceURI, 'path');
      trace.setAttribute('d', path.getAttribute('d'));
      trace.setAttribute('pathLength', '1');
      if (path.dataset.id) trace.dataset.id = path.dataset.id;
      trace.style.stroke = getComputedStyle(path).stroke;
      trace.classList.add('link-flow-trace');
      if (reverse) trace.classList.add('is-reverse');
      overlay.append(trace);
    }
    path.parentNode.append(overlay);
  }
}

// One shared group of ports per device; multiple links reuse the same buttons.
let portMotionFrame = 0;
function positionConnectionPorts() {
  const nodes = [...document.querySelectorAll('.map-node[data-kind="computer"]')];
  const now = performance.now(), reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  let moving = false;
  for (const node of nodes) {
    const rect = node.getBoundingClientRect(), cx = rect.left + rect.width / 2, cy = rect.top + rect.height / 2;
    const peers = nodes.filter(other => other !== node);
    if (!peers.length) continue;
    const linked = new Set(chatLinks.filter(value => value.source === node.dataset.id || value.target === node.dataset.id)
      .map(value => value.source === node.dataset.id ? value.target : value.source));
    const nearest = peers.slice().sort((a, b) => {
      const ar = a.getBoundingClientRect(), br = b.getBoundingClientRect();
      return Math.hypot(ar.left + ar.width / 2 - cx, ar.top + ar.height / 2 - cy) - Math.hypot(br.left + br.width / 2 - cx, br.top + br.height / 2 - cy);
    });
    const targets = peers.filter(other => linked.has(other.dataset.id));
    if (!targets.length) targets.push(nearest[0]);
    let tx = 0, ty = 0;
    for (const peer of targets) {
      const r = peer.getBoundingClientRect(), dx = r.left + r.width / 2 - cx, dy = r.top + r.height / 2 - cy;
      const distance = Math.max(1, Math.hypot(dx, dy));
      tx += dx / distance; ty += dy / distance;
    }
    if (Math.hypot(tx, ty) < 0.01) {
      const r = nearest[0].getBoundingClientRect(); tx = r.left + r.width / 2 - cx; ty = r.top + r.height / 2 - cy;
    }
    const scale = Math.max(rect.width, rect.height) * 2 / Math.max(0.01, Math.hypot(tx, ty));
    tx *= scale; ty *= scale;
    const size = rect.width + ':' + rect.height;
    let orbit = node.portOrbit;
    if (!orbit || orbit.size !== size) {
      const hx = rect.width / 2 + 24, hy = rect.height / 2 + 24;
      const radius = Math.min(hx, hy, (parseFloat(getComputedStyle(node).borderRadius) || 10) + 24);
      const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      path.setAttribute('d', `M${-hx + radius} ${-hy} H${hx - radius} A${radius} ${radius} 0 0 1 ${hx} ${-hy + radius} V${hy - radius} A${radius} ${radius} 0 0 1 ${hx - radius} ${hy} H${-hx + radius} A${radius} ${radius} 0 0 1 ${-hx} ${hy - radius} V${-hy + radius} A${radius} ${radius} 0 0 1 ${-hx + radius} ${-hy} Z`);
      const length = path.getTotalLength();
      orbit = node.portOrbit = { size, path, length, samples: Array.from({ length: 129 }, (_, index) => path.getPointAtLength(index * length / 128)), time: now };
    }
    const wrap = value => (value % orbit.length + orbit.length) % orbit.length;
    const delta = (a, b) => wrap(a - b + orbit.length / 2) - orbit.length / 2;
    let ideal = 0, best = Infinity;
    for (let index = 0; index < 128; index++) {
      const a = orbit.samples[index], b = orbit.samples[index + 1], dx = b.x - a.x, dy = b.y - a.y;
      const t = Math.max(0, Math.min(1, ((tx - a.x) * dx + (ty - a.y) * dy) / (dx * dx + dy * dy)));
      const distance = (a.x + dx * t - tx) ** 2 + (a.y + dy * t - ty) ** 2;
      if (distance < best) { best = distance; ideal = (index + t) * orbit.length / 128; }
    }
    if (orbit.phase === undefined) orbit.phase = orbit.target = ideal;
    if (Math.abs(delta(ideal, orbit.target)) > 7) orbit.target = ideal;
    const remaining = delta(orbit.target, orbit.phase);
    if (reduced || Math.abs(remaining) <= 0.1) orbit.phase = orbit.target;
    else { orbit.phase = wrap(orbit.phase + remaining * (1 - Math.exp(-Math.min(40, Math.max(1, now - orbit.time)) / 85))); moving = true; }
    orbit.time = now;
    const reverse = tx < 0 ? -1 : 1;
    for (const [index, operation] of ['sync', 'send', 'receive'].entries()) {
      const port = node.querySelector('.connection-port.' + operation);
      if (!port) continue;
      const position = orbit.phase + (index - 1) * 44 * reverse;
      const point = orbit.path.getPointAtLength(wrap(position));
      const before = orbit.path.getPointAtLength(wrap(position - 0.5)), after = orbit.path.getPointAtLength(wrap(position + 0.5));
      const length = Math.hypot(after.x - before.x, after.y - before.y);
      port.portNormal = { x: (after.y - before.y) / length, y: (before.x - after.x) / length };
      port.dataset.orbit = 'true';
      port.style.left = rect.width / 2 + point.x - node.clientLeft + 'px';
      port.style.top = rect.height / 2 + point.y - node.clientTop + 'px';
      port.querySelector('svg').style.transform = `rotate(${Math.atan2(port.portNormal.y, port.portNormal.x)}rad)`;
    }
  }
  if (moving && !portMotionFrame) portMotionFrame = requestAnimationFrame(() => { portMotionFrame = 0; renderLink(true); });
}

function renderLink(force = false) {
  if (drawingMap) return;
  renderMobileLinks();
  if (document.querySelector('.link-badge[data-dragging="true"]')) return;
  positionConnectionPorts();
  if (!force && document.querySelector(".link-badge:hover, .link-badge:focus-within")) return;
  const layers = new Map([...document.querySelectorAll(".chat-link")].map(node => [node.dataset.renderKey, node]));
  const geometry = Object.fromEntries([...document.querySelectorAll(".map-node")].map(node => {
    const rect = node.getBoundingClientRect();
    return [node.dataset.id, [rect.x, rect.y, rect.width, rect.height, node.portOrbit?.phase]];
  }));
  if(link && (data.deleted_chats || []).includes(link.entry.id))link=null;
  const links = chatLinks.filter(v => !(data.deleted_chats || []).includes(v.entry.id));
  if (link) {
    const existing = links.findIndex(v => v.entry.id === link.entry.id &&
      [v.source, v.target].sort().join(":") === [link.source, link.target].sort().join(":"));
    if (existing === -1) links.push(link); else links[existing] = link;
  }
  const lanes = new Map();
  links.forEach((value, index) => {
    const anchors = value.operation === "sync"
      ? [value.source, value.target].sort().join(":") + ":sync"
      : [value.source, value.target].join(":") + ":send";
    const lane = lanes.get(anchors) || 0;
    lanes.set(anchors, lane + 1);
    const key = JSON.stringify([value.entry.id, value.source, value.target]);
    const positionKey = [value.entry.id, ...[value.source, value.target].sort()].join(":");
    const signature = JSON.stringify([value, appearance(value.entry), linkPositions[positionKey], geometry[value.source], geometry[value.target], lane, index]);
    const existing = layers.get(key);
    layers.delete(key);
    if (existing && existing.dataset.renderSignature === signature) return;
    const layer = drawChatLink(value, index, lane, existing);
    if (layer) { layer.dataset.renderKey = key; layer.dataset.renderSignature = signature; }
    else if (existing) disposeChatLink(existing);
  });
  for (const layer of layers.values()) disposeChatLink(layer);
}

// Split the original cubic at the badge, preserving its shape and endpoint tangents.
function curveThroughBadge(points, midpoint) {
  const mix = (a, b, t) => ({ x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t });
  const split = t => {
    const a = mix(points[0], points[1], t), b = mix(points[1], points[2], t), c = mix(points[2], points[3], t);
    const d = mix(a, b, t), e = mix(b, c, t);
    return { a, c, d, e, joint: mix(d, e, t) };
  };
  let t = 0.5, best = Infinity;
  for (let i = 1; i < 100; i++) {
    const point = split(i / 100).joint;
    const distance = (point.x - midpoint.x) ** 2 + (point.y - midpoint.y) ** 2;
    if (distance < best) { best = distance; t = i / 100; }
  }
  for (let i = 0; i < 6; i++) {
    const { d, e, joint } = split(t);
    const dx = 3 * (e.x - d.x), dy = 3 * (e.y - d.y), squared = dx * dx + dy * dy;
    if (squared < 1e-8) break;
    t = Math.max(0.001, Math.min(0.999, t - ((joint.x - midpoint.x) * dx + (joint.y - midpoint.y) * dy) / squared));
  }
  const { a, c, d, e, joint } = split(t);
  return point => {
    const dx = point.x - joint.x, dy = point.y - joint.y;
    return `M${points[0].x} ${points[0].y} C${a.x} ${a.y} ${d.x + dx} ${d.y + dy} ${point.x} ${point.y} C${e.x + dx} ${e.y + dy} ${c.x} ${c.y} ${points[3].x} ${points[3].y}`;
  };
}

function drawChatLink(connection, index, lane = 0, existing = null) {
  const nodes = [...document.querySelectorAll(".map-node")];
  const from = nodes.find((n) => n.dataset.id === connection.source),
    to = nodes.find((n) => n.dataset.id === connection.target);
  if (!from || !to) return;
  const fromPort = from.querySelector(connection.operation === "sync" ? ".connection-port.sync" : ".connection-port.send");
  const toPort = to.querySelector(connection.operation === "sync" ? ".connection-port.sync" : ".connection-port.receive");
  if (!fromPort || !toPort) return;
  const base = $("#nodes").getBoundingClientRect();
  const start = fromPort.getBoundingClientRect(), end = toPort.getBoundingClientRect();
  const startNormal = fromPort.portNormal || { x: fromPort.classList.contains('sync') ? -1 : 1, y: 0 };
  const endNormal = toPort.portNormal || { x: toPort.classList.contains('sync') ? -1 : 1, y: 0 };
  const x1 = start.left + start.width / 2 + startNormal.x * (start.width / 2 + 2) - base.left,
    x2 = end.left + end.width / 2 + endNormal.x * (end.width / 2 + 2) - base.left,
    y1 = start.top + start.height / 2 + startNormal.y * (start.height / 2 + 2) - base.top,
    y2 = end.top + end.height / 2 + endNormal.y * (end.height / 2 + 2) - base.top;
  const dx = x2 - x1, dy = y2 - y1, distance = Math.max(1, Math.hypot(dx, dy));
  const reach = Math.max(45, Math.min(150, distance * 0.35)) + lane * 80;
  const controls = [{ x: x1, y: y1 }, { x: x1 + startNormal.x * reach, y: y1 + startNormal.y * reach },
    { x: x2 + endNormal.x * reach, y: y2 + endNormal.y * reach }, { x: x2, y: y2 }];
  const curve = `M${x1} ${y1} C${controls[1].x} ${controls[1].y} ${controls[2].x} ${controls[2].y} ${x2} ${y2}`;
  const layer = existing || document.createElement("div");
  layer.className = "chat-link";
  layer.dataset.source = connection.source;
  layer.dataset.target = connection.target;
  layer.dataset.operation = connection.operation;
  layer.id = index === 0 ? "chat-link" : "";
  layer.style.color = appearance(connection.entry).color || "var(--ink)";
  if (!existing) layer.innerHTML =
    '<svg class="connection-lines" aria-hidden="true"><defs><marker id="link-arrow-' + index + '" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="10" markerHeight="10" markerUnits="userSpaceOnUse" orient="auto-start-reverse"><path d="M1 1L9 5L1 9Z"/></marker></defs><path d="M' +
    curve.slice(1) +
    '" stroke-dasharray="' +
    (connection.operation === "sync" ? "none" : "7 6") +
    '" marker-end="url(#link-arrow-' + index + ')" ' +
    (connection.operation === "sync" ? 'marker-start="url(#link-arrow-' + index + ')"' : "") +
    '/></svg><div class="link-badge" title="' +
    esc(chatLabel(connection.entry)) +
    '">' +
    badge(connection.entry) +
    "</div>";
  const marker = layer.querySelector(".link-badge");
  marker.dataset.linkKey = JSON.stringify([connection.entry.id, connection.source, connection.target]);
  if (!existing) marker.replaceChildren();
  const name = chatLabel(connection.entry);
  marker.title = name;
  const main = marker.querySelector(".link-main") || document.createElement("button");
  main.className = "link-main";
  main.type = "button";
  const badgeHtml = badge(connection.entry);
  if (main.dataset.badgeHtml !== badgeHtml) { main.innerHTML = badgeHtml; main.dataset.badgeHtml = badgeHtml; }
  main.title = name;
  main.setAttribute("aria-label", OaclisSettings.text("Sposta ") + name);
  main.title = OaclisSettings.text("Trascina ") + name;
  const open = marker.querySelector(".link-open") || document.createElement("button");
  open.type = "button"; open.className = "link-open icon-button";
  open.setAttribute("aria-label", OaclisSettings.text("Apri ") + name); open.title = OaclisSettings.text("Apri console");
  open.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true">' + icons.terminal + '</svg>';
  open.onclick = () => chooseChatTerminal(connection, open);
  const edit = marker.querySelector(".link-edit") || document.createElement("button");
  edit.type = "button";
  edit.className = "link-edit icon-button";
  edit.setAttribute("aria-label", OaclisSettings.text("Modifica ") + name);
  edit.title = OaclisSettings.text("Modifica chat");
  edit.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m15 4 5 5-11 11-6 1 1-6zm-2 2 5 5"/></svg>';
  edit.onclick = guard(async () => {
    const cached = connection.source === sourceId ? visibleChats : connection.source === "master" ? data.chats : [];
    const panel = editChat(cached.find(c => c.id === connection.entry.id) || connection.entry, edit, connection.source, connection);
    try {
      const result = await api("catalog", { device_id: connection.source });
      const fresh = result.chats.find(c => c.id === connection.entry.id);
      if (fresh && panel.isConnected) renderScopeDetails(panel, fresh);
    } catch (_) { /* Cached details remain usable while the source is unreachable. */ }
  });
  const play = marker.querySelector(".link-play") || document.createElement("button");
  play.type = "button"; play.className = "link-play icon-button";
  const phase = connection.flow?.phase;
  play.hidden = false;
  play.disabled = data.paused || phase !== "ready" || marker.playPending === true || connection.flow?.requested === true ||
    connection.operation !== "sync" && connection.flow?.initialized ||
    [connection.source,connection.target].some(id => id !== "master" && data.connections[id] === "offline");
  play.dataset.ready = String(!play.disabled);
  play.title = play.disabled ? (phase === "transferring" || marker.playPending || connection.flow?.requested ? OaclisSettings.text("Sincronizzazione in corso") : OaclisSettings.text("Sincronizzazione non disponibile")) : OaclisSettings.text("Sincronizza verso la copia rimasta indietro");
  play.setAttribute("aria-label", play.title);
  play.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8 5 11 7-11 7Z"/></svg>';
  play.onclick = guard(async () => {
    play.disabled = true;
    marker.playPending = true;
    updateBadgeMotion(marker, connection);
    try { chatLinks = await saveLinkChange(() => api("links/play", connection)); renderLink(true); }
    finally { marker.playPending = false; if (play.isConnected) { renderLink(true); updateBadgeMotion(marker, chatLinks.find(value => value.entry.id === connection.entry.id && value.source === connection.source && value.target === connection.target) || connection); } }
  });
  marker.title = name + (connection.flow?.error ? " · " + connection.flow.error : phase === "conflict" ? OaclisSettings.text(" · Le copie divergono: scegliere quale mantenere") : phase === "offline" ? OaclisSettings.text(" · In attesa del dispositivo") : "");
  const url = appearance(connection.entry).url || "";
  let web = marker.querySelector(".link-web");
  if (url) {
    try {
      const parsed = new URL(url);
      if (!["http:", "https:"].includes(parsed.protocol) || parsed.username || parsed.password) throw new Error(OaclisSettings.text("URL non valido"));
      if (!web) { web = document.createElement("a"); web.className = "link-web icon-button"; marker.append(web); }
      web.href = parsed.href; web.target = "_blank"; web.rel = "noopener noreferrer";
      web.title = OaclisSettings.text("Apri pagina web"); web.setAttribute("aria-label", OaclisSettings.text("Apri pagina web di ") + name);
      web.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true">' + icons.link + '</svg>';
      web.inert = ["transferring", "paused", "finishing"].includes(marker.dataset.motionPhase);
    } catch (_) { web?.remove(); }
  } else { web?.remove(); }
  if (!existing) marker.append(main, edit, play, open);
  else if (!play.isConnected) marker.append(play);
  main.querySelector(".chat-badge").style.color = appearance(connection.entry).color || "var(--ink)";
  marker.style.color = appearance(connection.entry).color || "var(--ink)";
  if (!existing) {
    $("#nodes").prepend(layer);
    marker.badgeMotion = new OaclisBadgeMotion(marker, { chat:icons.chat, paths:icons.folder, apps:icons.box });
  }
  updateBadgeMotion(marker, connection);
  const path = layer.querySelector(".connection-lines > path");
  const arrow = layer.querySelector("marker");
  arrow.id = 'link-arrow-' + index;
  path.setAttribute('marker-end', 'url(#' + arrow.id + ')');
  if (connection.operation === 'sync') path.setAttribute('marker-start', 'url(#' + arrow.id + ')');
  else path.removeAttribute('marker-start');
  path.setAttribute('stroke-dasharray', connection.operation === 'sync' ? 'none' : '7 6');
  path.setAttribute('d', curve);
  const midpoint = path.getPointAtLength(path.getTotalLength() / 2);
  const bend = curveThroughBadge(controls, midpoint);
  const updateCurve = () => {
    path.setAttribute('d', bend({ x: parseFloat(marker.style.left), y: parseFloat(marker.style.top) }));
    layer.querySelectorAll('.link-flow-trace').forEach(trace => trace.setAttribute('d', path.getAttribute('d')));
  };
  const key = [connection.entry.id, ...[connection.source, connection.target].sort()].join(":");
  const offset = linkPositions[key] || { x: 0, y: 0 };
  marker.style.left = midpoint.x + offset.x + "px";
  marker.style.top = midpoint.y + offset.y + "px";
  updateCurve();
  const save = () => {
    linkPositions[key] = { x: marker.offsetLeft - midpoint.x, y: marker.offsetTop - midpoint.y };
    linkPositionSaving = true;
    api("link-position", { id: connection.entry.id, source: connection.source, target: connection.target, ...linkPositions[key] })
      .catch(e => notice(e.message)).finally(() => { linkPositionSaving = false; });
  };
  main.onpointerdown = event => {
    if (event.button !== 0) return;
    event.preventDefault();
    const start = { x: event.clientX, y: event.clientY, left: marker.offsetLeft, top: marker.offsetTop };
    marker.dataset.dragging = "true";
    main.setPointerCapture(event.pointerId);
    main.onpointermove = e => {
      if (!main.hasPointerCapture(e.pointerId)) return;
      marker.style.left = Math.max(22, Math.min(start.left + e.clientX - start.x, $("#nodes").clientWidth - 22)) + "px";
      marker.style.top = Math.max(22, Math.min(start.top + e.clientY - start.y, $("#nodes").clientHeight - 22)) + "px";
      updateCurve();
    };
    main.onpointerup = e => {
      save(); delete marker.dataset.dragging;
      if (main.hasPointerCapture(e.pointerId)) main.releasePointerCapture(e.pointerId);
    };
    main.onpointercancel = main.onlostpointercapture = () => delete marker.dataset.dragging;
  };
  main.onkeydown = event => {
    const directions = { ArrowLeft: [-10, 0], ArrowRight: [10, 0], ArrowUp: [0, -10], ArrowDown: [0, 10] };
    if (!directions[event.key]) return;
    event.preventDefault();
    const [x, y] = directions[event.key];
    marker.style.left = marker.offsetLeft + x + "px";
    marker.style.top = marker.offsetTop + y + "px";
    updateCurve();
    save();
  };
  return layer;
}

function acceptsChatDrop(source, target, operation) {
  return source !== target && (source === "master" || target === "master") &&
    (operation === "sync" || operation === "receive");
}

function renderLibrary() {
  const search = $("#search").value.toLocaleLowerCase();
  const focused = document.activeElement?.dataset.chatId;
  const entries = visibleChats.filter(
    (c) => !(data.deleted_chats || []).includes(c.id) && (showHiddenChats || !data.chat_settings?.[c.id]?.hidden) && (
      chatLabel(c).toLocaleLowerCase().includes(search) ||
      c.name.toLocaleLowerCase().includes(search) ||
      c.id.toLocaleLowerCase().includes(search)),
  );
  $("#library-list").replaceChildren();
  if (!entries.length) {
    $("#library-list").innerHTML =
      '<p class="muted">' +
      (search ? OaclisSettings.text("Nessun risultato.") : OaclisSettings.text("Nessuna chat disponibile.")) +
      "</p>";
    return;
  }
  for (const entry of entries) {
    const button = document.createElement("button");
    button.className = "library-item";
    button.type = "button";
    button.dataset.kind = "chat";
    button.dataset.chatId = entry.id;
    button.draggable = !data.chat_settings?.[entry.id]?.hidden;
    button.innerHTML =
      badge(entry) +
      "<span><strong>" +
      esc(chatLabel(entry)) +
      "</strong><small>" +
      esc(entry.id) +
      "</small></span>";
    button.querySelector(".chat-badge").style.color =
      appearance(entry).color || "var(--ink)";
    button.ondragstart = (e) => {
      drag = { entry, sourceId };
      e.dataTransfer.setData("application/oaclis", entry.id);
      e.dataTransfer.effectAllowed = "copy";
      const preview = button.cloneNode(true);
      preview.id = "chat-drag-preview";
      preview.classList.add("chat-drag-preview");
      preview.style.width = button.offsetWidth + "px";
      preview.style.borderColor = appearance(entry).color;
      document.body.append(preview);
      e.dataTransfer.setDragImage(preview, 24, 24);
      document.querySelectorAll(".connection-port").forEach(port => {
        if (acceptsChatDrop(drag.sourceId, port.closest(".map-node").dataset.id, port.dataset.operation))
          port.dataset.dropReady = "true";
        else port.dataset.dropBlocked = "true";
      });
    };
    button.ondragend = () => {
      drag = null;
      $("#chat-drag-preview")?.remove();
      document.querySelectorAll(".connection-port").forEach(port => { delete port.dataset.dropReady; delete port.dataset.dropOver; delete port.dataset.dropBlocked; });
      document
        .querySelectorAll(".drop-target")
        .forEach((n) => n.classList.remove("drop-target"));
    };
    const row = document.createElement("div");
    row.className = "library-row"; row.dataset.sourceDevice = sourceId;
    const rowColor = appearance(entry).color;
    if (rowColor)
      row.style.backgroundColor =
        "color-mix(in srgb, " + rowColor + " 14%, transparent)";
    const edit = document.createElement("button");
    edit.className = "icon-button";
    edit.setAttribute("aria-label", OaclisSettings.text("Personalizza ") + chatLabel(entry));
    edit.innerHTML =
      '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m15 4 5 5-11 11-6 1 1-6zm-2 2 5 5"/></svg>';
    edit.onclick = () => editChat(entry, edit);
    row.classList.toggle("hidden-chat", data.chat_settings?.[entry.id]?.hidden === true);
    const remove=document.createElement("button");remove.type="button";
    remove.className="icon-button delete-chat";remove.title=OaclisSettings.text("Elimina definitivamente");
    remove.setAttribute("aria-label",OaclisSettings.text("Elimina definitivamente ")+chatLabel(entry));
    remove.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7"/></svg>';
    remove.onclick=guard(()=>deleteChat(entry));
    row.append(button, edit, remove);
    $("#library-list").append(row);
    if (focused === entry.id) button.focus({ preventScroll: true });
  }
}

function mapLayout() {
  const width = $("#map").clientWidth,
    height = $("#map").clientHeight,
    count = devices().length;
  const nodeWidth = width <= 640 ? 160 : 204,
    gap = 88;
  const columns = Math.max(
    1,
    Math.min(
      Math.ceil(Math.sqrt(count)),
      Math.floor((width - 110 + gap) / (nodeWidth + gap)),
    ),
  );
  const rows = Math.ceil(count / columns),
    gridHeight = rows * 90 + (rows - 1) * gap;
  return {
    nodeWidth,
    gap,
    columns,
    gridHeight,
    canvasHeight: height,
    x: Math.max(55, (width - (columns * nodeWidth + (columns - 1) * gap)) / 2),
    y: Math.max(110, (height - gridHeight) / 2),
  };
}

function defaultPosition(index) {
  const layout = mapLayout();
  return {
    x: layout.x + (index % layout.columns) * (layout.nodeWidth + layout.gap),
    y: layout.y + Math.floor(index / layout.columns) * (90 + layout.gap),
  };
}

function place(node, p) {
  const edge = node.dataset.kind === 'computer' ? 55 : 0;
  node.style.left =
    Math.max(55, Math.min(p.x, $("#map").clientWidth - node.offsetWidth - 55)) +
    "px";
  node.style.top =
    Math.max(
      edge,
      Math.min(p.y, $("#nodes").clientHeight - node.offsetHeight - edge),
    ) + "px";
  renderLink();
}

function savePositions() {
  clearTimeout(positionTimer);
  positionsSaving = true;
  const snapshot = JSON.parse(JSON.stringify(positions));
  positionTimer = setTimeout(() => api("positions", snapshot).catch(e => notice(e.message)).finally(() => { positionsSaving = false; }), 120);
}

function savePosition(node) {
  positions[node.dataset.id] = { x: node.offsetLeft, y: node.offsetTop };
  savePositions();
}

function followMapMotion() {
  cancelAnimationFrame(mapMotionFrame);
  const duration = getComputedStyle(document.documentElement).getPropertyValue("--duration-ui").trim();
  const milliseconds = parseFloat(duration) * (duration.endsWith("ms") ? 1 : 1000);
  const until = performance.now() + (matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : milliseconds);
  const frame = () => {
    renderLink(true);
    if (performance.now() < until) mapMotionFrame = requestAnimationFrame(frame);
  };
  mapMotionFrame = requestAnimationFrame(frame);
}

function renderMap(animate = false) {
  drawingMap = true;
  const previous = animate ? Object.fromEntries([...document.querySelectorAll(".map-node")].map(n => [n.dataset.id, { x: n.offsetLeft, y: n.offsetTop }])) : {};
  $("#nodes").querySelectorAll(".map-node").forEach(node => node.remove());
  $("#nodes").style.height = mapLayout().canvasHeight + "px";
  devices().forEach((device, index) => {
    const mobile = device.kind === 'mobile';
    const node = document.createElement("div");
    node.dataset.kind = mobile ? 'mobile' : 'computer';
    node.className = "map-node";
    node.dataset.id = device.id;
    node.tabIndex = 0;
    node.setAttribute("role", mobile ? "group" : "button");
    node.innerHTML =
      '<span class="device-symbol">' + iconBody(mobile ? "mobile" : data.appearance?.["device:" + device.id]?.icon ?? "desktop", device.name) +
      (!mobile && device.id !== "master" && data.ssh_settings?.[device.id]?.in_lan !== true ? OaclisSettings.html('<span class="remote-badge" role="img" aria-label="Probabilmente remoto" title="Probabilmente remoto"><svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="4" ry="9"/><path d="M3 12h18"/></svg></span>') : '') + '</span>' +
      '<span class="node-name">'  +
      esc(device.name) +
      '</span><i class="status-dot" aria-hidden="true"></i>';
    const color = data.appearance?.["device:" + device.id]?.color ||
      (device.id === "master" ? null : automaticColor(device.id));
    if (color) {
      node.style.setProperty("--device-color", color);
      node.classList.add("colored-device");
    }
    const actions = document.createElement("nav");
    actions.className = "device-actions";
    actions.setAttribute("aria-label", "Comandi " + device.name);
    node.append(actions);
    if (!mobile) {
    const customize = document.createElement("button");
    customize.className = "node-color icon-button";
    customize.setAttribute("aria-label", OaclisSettings.text("Modifica ") + device.name);
    customize.title = OaclisSettings.text("Modifica dispositivo");
    customize.innerHTML =
      '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m15 4 5 5-11 11-6 1 1-6zm-2 2 5 5"/></svg>';
    customize.onclick = (e) => {
      e.stopPropagation();
      editDevice(device, customize);
    };
    actions.append(customize);
    for (const [action, label, shape] of [
      ["restart", OaclisSettings.text("Riavvia Oaclis"), '<path d="M20 7v5h-5M20 12a8 8 0 1 0-2 5"/>'],
      ["toggle-awake", OaclisSettings.text("Sempre attivo"), '<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l2 2m10 10 2 2M5 19l2-2M17 7l2-2"/>'],
    ]) {
      const button = document.createElement("button");
      button.className = "node-color node-" + action + " icon-button";
      button.title = label; button.setAttribute("aria-label", label + " · " + device.name);
      button.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true">' + shape + '</svg>';
      if (action === "toggle-awake") button.setAttribute("aria-pressed", "mixed");
      const update = result => {
        result.observed_at ??= Date.now();
        data.device_runtime ??= {}; data.device_runtime[device.id] = result;
        button.setAttribute("aria-pressed", String(result.active));
        button.title = (result.active ? OaclisSettings.text("Standby bloccato") : OaclisSettings.text("Standby consentito")) + (result.error ? " · " + result.error : "");
      };
      if (action === "toggle-awake" && data.device_runtime?.[device.id]) update(data.device_runtime[device.id]);
      button.onclick = guard(async event => {
        event.stopPropagation();
        if (action === "restart" && !window.confirm(OaclisSettings.text("Riavviare Oaclis su “") + device.name + "”?")) return;
        button.disabled = true; button.dataset.busy = "true";
        if (action === "toggle-awake") button.title = button.getAttribute("aria-pressed") === "true" ? OaclisSettings.text("Disattivazione blocco standby…") : OaclisSettings.text("Attivazione blocco standby…");
        try {
          const result = await api("device/control", { device_id: device.id, action });
          if (action === "toggle-awake") update(result);
          else if (device.id === "master") window.setTimeout(() => window.location.reload(), 1500);
          else notice(OaclisSettings.text("Riavvio Oaclis richiesto su ") + device.name);
        } finally { delete button.dataset.busy; renderConnections(); }
      });
      actions.append(button);
    }
    if (device.id !== "master" && data.role === "master") {
      const desktop = document.createElement("a");
      desktop.className = "node-color node-desktop icon-button";
      desktop.title = OaclisSettings.text("Desktop remoto");
      desktop.setAttribute("aria-label", OaclisSettings.text("Desktop remoto · ") + device.name);
      desktop.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8m-4-4v4"/></svg>';
      desktop.hidden = true;
      desktop.onclick = event => {
        event.stopPropagation();
        if (desktop.getAttribute("aria-disabled") === "true") event.preventDefault();
      };
      actions.append(desktop);
    }
    if (device.id !== "master" && data.role === "master") {
      const version = document.createElement("button");
      version.className = "node-color node-version icon-button";
      version.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 5 7 14L19 5"/></svg>';
      version.type = "button";
      version.onclick = guard(async event => {
        event.stopPropagation();
        version.dataset.checking = "true"; version.disabled = true;
        let current;
        try {
          current = await api("device/control", {device_id:device.id,action:"status"});
          data.device_runtime ??= {}; data.device_runtime[device.id] = current;
        } finally { delete version.dataset.checking; renderConnections(); }
        if (!current.build?.source_id) throw new Error(OaclisSettings.text("Il dispositivo non comunica la versione installata"));
        if (current.build.source_id === data.build?.source_id) { notice(OaclisSettings.text("Oaclis è allineato su ") + device.name); return; }
        version.dataset.busy = "true"; version.disabled = true;
        try {
          const result = await api("device/control", {device_id:device.id,action:"update"});
          result.observed_at ??= Date.now();
          data.device_runtime ??= {}; data.device_runtime[device.id] = result;
          notice(OaclisSettings.text("Aggiornamento Oaclis avviato su ") + device.name);
        } catch (error) {
          console.error("Oaclis update:", error);
          notice(OaclisSettings.text("Aggiornamento non riuscito"));
        } finally { delete version.dataset.busy; renderConnections(); }
      });
      actions.append(version);
    }
    } else {
      const access = document.createElement('button');
      access.className = 'node-color icon-button'; access.type = 'button';
      access.title = OaclisSettings.text('QR accesso'); access.setAttribute('aria-label', OaclisSettings.text('QR accesso · ') + device.name);
      access.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 3h6v6H3zM15 3h6v6h-6zM3 15h6v6H3zM15 15h2v2h-2zm4 0h2v6h-6v-2"/></svg>';
      access.onclick = event => { event.stopPropagation(); openMobileAccess(access); };
      actions.append(access);
      const remove = document.createElement('button');
      remove.className = 'node-color icon-button'; remove.type = 'button';
      remove.title = OaclisSettings.text('Elimina telefono'); remove.setAttribute('aria-label', OaclisSettings.text('Elimina ') + device.name);
      remove.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7"/></svg>';
      remove.onclick = guard(async event => { event.stopPropagation(); await removeMobile(device); });
      actions.append(remove);
    }
    $("#nodes").append(node);
    place(node, previous[device.id] || positions[device.id] || defaultPosition(index));
    if (!mobile) {
      for (const [operation, label] of [
        ["sync", OaclisSettings.text("Sincronizzazione bidirezionale")],
        ["send", OaclisSettings.text("Invia a questo dispositivo")],
        ["receive", OaclisSettings.text("Ricevi da questo dispositivo")],
      ]) {
        const port = document.createElement("button");
        port.className = "connection-port " + operation;
        port.dataset.operation = operation;
        port.setAttribute("aria-label", label + " · " + device.name);
        port.title = label;
        const paths = {
          sync: "M4 8h16m-5-4 5 4-5 4M20 16H4m5-4-5 4 5 4",
          send: "M4 12h16m-5-5 5 5-5 5",
          receive: "M20 12H4m5-5-5 5 5 5",
        };
        port.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="' + paths[operation] + '"/></svg>';
        const local = device.id === "master";
  const objectId = device.id || "device-" + crypto.randomUUID();
        if (local) port.title = operation === "send" ? OaclisSettings.text("Invia a un dispositivo") : operation === "receive" ? OaclisSettings.text("Ricevi da un dispositivo") : label;
        const peer = () => !local ? "master" :
          sourceId && sourceId !== "master" ? sourceId : data.devices.length === 1 ? data.devices[0].id : null;
        const origin = () => operation === "receive" ? peer() : device.id;
        const target = id => id === device.id ? peer() || undefined : device.id;
        const transferOperation = operation === "receive" ? "send" : operation;
        const accepts = id => acceptsChatDrop(id, device.id, operation);
        port.onclick = guard(async e => {
          e.stopPropagation();
          if (selected && accepts(sourceId))
            selectChat(selected, target(sourceId), transferOperation);
          else {
            const id = origin();
            if (!id) { notice(OaclisSettings.text("Seleziona il dispositivo da cui ricevere la chat.")); return; }
            await openDevice(id);
          }
        });
        port.ondragover = e => {
          e.stopPropagation();
          if (!drag || !accepts(drag.sourceId)) return;
          e.preventDefault();
          port.dataset.dropOver = "true";
        };
        port.ondragleave = () => delete port.dataset.dropOver;
        port.ondrop = e => {
          delete port.dataset.dropOver;
          e.preventDefault();
          e.stopPropagation();
          if (!drag) return;
          if (!accepts(drag.sourceId)) {
            notice(OaclisSettings.text("Trascina su Ricevi o Bidirezionale di un altro dispositivo compatibile."));
            return;
          }
          sourceId = drag.sourceId;
          selectChat(drag.entry, target(sourceId), transferOperation);
        };
        node.append(port);
      }
    }
    node.onpointerenter = event => { if (event.pointerType !== 'touch') previewLinkFlow(node); };
    node.onpointerleave = () => previewLinkFlow(null);
    node.onfocus = () => previewLinkFlow(node);
    node.onblur = () => previewLinkFlow(null);
    let start,
      moved = false;
    node.onpointerdown = (e) => {
      if (e.button !== 0 || e.target.closest("button, a"))
        return;
      start = {
        x: e.clientX,
        y: e.clientY,
        left: node.offsetLeft,
        top: node.offsetTop,
      };
      moved = false;
      node.dataset.dragging = "true";
      node.setPointerCapture(e.pointerId);
    };
    node.onpointermove = (e) => {
      if (!start) return;
      const dx = e.clientX - start.x,
        dy = e.clientY - start.y;
      if (Math.abs(dx) + Math.abs(dy) > 4) moved = true;
      if (moved) place(node, { x: start.left + dx, y: start.top + dy });
    };
    node.onpointerup = () => {
      if (start && moved) savePosition(node);
      start = null;
      delete node.dataset.dragging;
    };
    node.onpointercancel = node.onlostpointercapture = () => {
      start = null;
      delete node.dataset.dragging;
    };
    node.onclick = guard(async () => {
      if (!moved) { if (mobile) await openMobileChats(device, node); else await openDevice(device.id); }
    });
    node.onkeydown = guard(async (e) => {
      if (e.target !== node) return;
      if (["Enter", " "].includes(e.key)) {
        e.preventDefault();
        if (mobile) await openMobileChats(device, node); else await openDevice(device.id);
      }
      const offsets = {
        ArrowLeft: [-10, 0],
        ArrowRight: [10, 0],
        ArrowUp: [0, -10],
        ArrowDown: [0, 10],
      };
      if (offsets[e.key]) {
        e.preventDefault();
        place(node, {
          x: node.offsetLeft + offsets[e.key][0],
          y: node.offsetTop + offsets[e.key][1],
        });
        savePosition(node);
      }
    });
  });
  drawingMap = false;
  renderConnections();
  if (animate) {
    $("#nodes").getBoundingClientRect();
    requestAnimationFrame(() => {
      [...document.querySelectorAll(".map-node")].forEach((node, index) => {
        if (node.dataset.dragging !== "true") place(node, positions[node.dataset.id] || defaultPosition(index));
      });
      followMapMotion();
    });
  }
}

function renderConnections() {
  renderLink();
  document.querySelectorAll(".map-node").forEach((node) => {
    const id = node.dataset.id;
    const mobile = data.mobile_devices?.find(d => d.id === id);
    const state = mobile ? mobile.active ? 'online' : 'offline' :
      id === "master" ? "online" : data.connections[id] || "unknown";
    node.className =
      "map-node " +
      state +
      (id === sourceId ? " selected" : "") +
      (id !== "master" || data.appearance?.["device:" + id]?.color ? " colored-device" : "");
    const label = mobile ? mobile.active ? OaclisSettings.text('Attivo') : OaclisSettings.text('Non attivo') : {
      online: OaclisSettings.text("raggiungibile"),
      offline: OaclisSettings.text("non raggiungibile"),
      checking: OaclisSettings.text("verifica in corso"),
      unknown: OaclisSettings.text("non verificato"),
    }[state];
    node.querySelectorAll(".node-restart, .node-toggle-awake").forEach(button => { button.disabled = state !== "online" || button.dataset.busy === "true"; });
    const awake = node.querySelector(".node-toggle-awake");
    const runtime = data.device_runtime?.[id];
    if (awake && !awake.dataset.busy) {
      const known = state === "online" && runtime?.available !== false && typeof runtime?.active === "boolean";
      awake.setAttribute("aria-pressed", known ? String(runtime.active) : "mixed");
      awake.title = (known ? runtime.active ? OaclisSettings.text("Standby bloccato") : OaclisSettings.text("Standby consentito") : OaclisSettings.text("Stato standby non disponibile")) + (runtime?.error ? " · " + runtime.error : "");
      awake.setAttribute("aria-label", deviceName(id) + " · " + awake.title);
    }
    const desktop = node.querySelector(".node-desktop");
    if (desktop) {
      const desktopId = runtime?.desktop?.id;
      const known = typeof desktopId === "string" && /^\d{6,16}$/.test(desktopId);
      desktop.hidden = !known;
      const enabled = known && state === "online";
      desktop.setAttribute("aria-disabled", String(!enabled));
      desktop.tabIndex = enabled ? 0 : -1;
      if (enabled) desktop.href = "rustdesk://connection/new/" + desktopId;
      else desktop.removeAttribute("href");
    }
    const version = node.querySelector(".node-version");
    if (version) {
      const runtime = data.device_runtime?.[id];
      const build = runtime?.build;
      const pending = version.dataset.busy || runtime?.update?.status === "running";
      const known = build?.source_id && data.build?.source_id;
      const aligned = known && build.source_id === data.build.source_id;
      const status = pending ? "updating" : known ? aligned ? "aligned" : "outdated" : "unknown";
      version.dataset.state = status;
      version.disabled = state !== "online" || !!pending || !!version.dataset.checking;
      version.title = build?.version || "";
      version.setAttribute("aria-label", build?.version || OaclisSettings.text("Versione non disponibile"));
    }
    node.title = deviceName(id) + " · " + label;
    node.setAttribute(
      "aria-label",
      deviceName(id) +
        ", " +
        label +
        (mobile ? OaclisSettings.text(". Invio per scegliere le chat, frecce per spostare.") : OaclisSettings.text(". Invio per le chat, frecce per spostare.")),
    );
  });
}

async function selectChat(entry, targetId, operation = "copy") {
  const association = { source: sourceId, target: targetId, operation, entry: { id: entry.id, name: entry.name },
    scopes: data.chat_settings?.[entry.id]?.scopes || { chat: true, paths: true, apps: true }, transfer_pending: true };
  try {
    await saveLinkChange(async () => {
      chatLinks = await api("links/save", association);
      link = null;
      renderLink(true);
    });
  } catch (error) { notice(OaclisSettings.text("Associazione non salvata: ") + error.message); }
}


function mobileLinkCurve(from, to, base, radius = 0) {
  const overlapX = [Math.max(from.left + radius, to.left + radius), Math.min(from.right - radius, to.right - radius)];
  const overlapY = [Math.max(from.top + radius, to.top + radius), Math.min(from.bottom - radius, to.bottom - radius)];
  const sides = [from, to].map((rect, index) => {
    const other = index ? from : to;
    const inset = Math.min(radius, rect.width / 2, rect.height / 2);
    const x = overlapX[0] <= overlapX[1] ? (overlapX[0] + overlapX[1]) / 2 : Math.max(rect.left + inset, Math.min(rect.right - inset, other.left + other.width / 2));
    const y = overlapY[0] <= overlapY[1] ? (overlapY[0] + overlapY[1]) / 2 : Math.max(rect.top + inset, Math.min(rect.bottom - inset, other.top + other.height / 2));
    return [{x:rect.left - 2,y,nx:-1,ny:0}, {x:rect.right + 2,y,nx:1,ny:0},
      {x,y:rect.top - 2,nx:0,ny:-1}, {x,y:rect.bottom + 2,nx:0,ny:1}];
  });
  let best;
  for (const a of sides[0]) for (const b of sides[1]) {
    const dx = b.x - a.x, dy = b.y - a.y, distance = Math.hypot(dx, dy);
    const facing = a.nx * dx + a.ny * dy >= 0 && b.nx * dx + b.ny * dy <= 0;
    if (!best || facing && !best.facing || facing === best.facing && distance < best.distance) best = {a,b,distance,facing};
  }
  const {a,b,distance} = best, reach = Math.min(150, distance * 0.35);
  return `M${a.x - base.left} ${a.y - base.top}C${a.x + a.nx * reach - base.left} ${a.y + a.ny * reach - base.top} ${b.x + b.nx * reach - base.left} ${b.y + b.ny * reach - base.top} ${b.x - base.left} ${b.y - base.top}`;
}

function renderMobileLinks() {
  let svg = document.querySelector('#mobile-links');
  const phones = data.mobile_devices || [];
  const masterNode = [...document.querySelectorAll('.map-node')].find(n => n.dataset.id === 'master');
  if (!phones.length || !masterNode) { svg?.remove(); return; }
  if (!svg) {
    svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.id = 'mobile-links'; svg.setAttribute('aria-hidden', 'true');
    svg.innerHTML = '<defs></defs>';
    $('#nodes').prepend(svg);
  }
  const base = $('#nodes').getBoundingClientRect();
  const radius = parseFloat(getComputedStyle(masterNode).borderTopLeftRadius) || 0;
  const keep = new Set();
  for (const device of phones) {
    const node = [...document.querySelectorAll('.map-node')].find(n => n.dataset.id === device.id);
    if (!node) continue;
    keep.add(device.id);
    let path = [...svg.querySelectorAll(':scope > path')].find(p => p.dataset.id === device.id);
    if (!path) {
      path = document.createElementNS(svg.namespaceURI, 'path'); path.dataset.id = device.id;
      const marker = document.createElementNS(svg.namespaceURI, 'marker'); marker.dataset.id = device.id;
      marker.id = 'mobile-arrow-' + device.id; marker.setAttribute('viewBox','0 0 10 10'); marker.setAttribute('refX','9'); marker.setAttribute('refY','5');
      marker.setAttribute('markerWidth','10'); marker.setAttribute('markerHeight','10'); marker.setAttribute('markerUnits','userSpaceOnUse'); marker.setAttribute('orient','auto');
      marker.innerHTML = '<path d="M1 1L9 5L1 9Z"/>'; svg.querySelector('defs').append(marker);
      path.setAttribute('marker-end','url(#' + marker.id + ')'); svg.append(path);
    }
    path.setAttribute('d', mobileLinkCurve(masterNode.getBoundingClientRect(), node.getBoundingClientRect(), base, radius));
    svg.querySelectorAll('.link-flow-trace[data-id="' + CSS.escape(device.id) + '"]').forEach(trace => trace.setAttribute('d', path.getAttribute('d')));
    path.dataset.active = String(device.active === true);
    [...svg.querySelector('defs').children].find(m => m.dataset.id === device.id).dataset.active = path.dataset.active;
  }
  for (const item of [...svg.querySelectorAll(':scope > path, defs > marker')]) if (!keep.has(item.dataset.id)) item.remove();
}
