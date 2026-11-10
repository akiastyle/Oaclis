// map.js v4.134.3544
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

// Reuse status events; a stationary connection has no animated overlay.
function updateLinkFlow(path, directions) {
  if (!path) return;
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) directions = [];
  const signature = directions.join(':');
  if (path.flowDirections === signature) return;
  path.flowDirections = signature;
  path.flowOverlay?.remove();
  if (!directions.length) return;
  const overlay = document.createElementNS(path.namespaceURI, 'g');
  overlay.classList.add('link-flow-overlay');
  path.flowOverlay = overlay;
  for (const reverse of directions) {
    const trace = document.createElementNS(path.namespaceURI, 'path');
    trace.setAttribute('d', path.getAttribute('d'));
    trace.setAttribute('pathLength', '1');
    if (path.dataset.id) trace.dataset.id = path.dataset.id;
    trace.classList.add('link-flow-trace');
    if (reverse) trace.classList.add('is-reverse');
    overlay.append(trace);
  }
  path.parentNode.append(overlay);
}

function renderActiveLinkFlows() {
  const online = id => id === 'master' || data.connections?.[id] === 'online';
  for (const layer of document.querySelectorAll('.chat-link')) {
    const connection = chatLinks.find(value => JSON.stringify([value.entry.id, value.source, value.target]) === layer.dataset.renderKey);
    let directions = [];
    if (connection && online(connection.source) && online(connection.target) && !data.paused) {
      const pair = [connection.entry.id, ...[connection.source, connection.target].sort()].join(':');
      const current = data.job?.link_key === pair ? data.job : [data.transfers?.[connection.source + ':' + connection.entry.id], data.transfers?.[connection.target + ':' + connection.entry.id]].find(record => record && record.chat_id === connection.entry.id && [record.source_device_id, record.device_id].sort().join(':') === [connection.source, connection.target].sort().join(':'));
      const running = connection.flow?.phase === 'transferring' || current?.status === 'running';
      const matches = !current?.generation || !connection.flow?.generation || current.generation === connection.flow.generation;
      if (running && matches && !['paused', 'failed', 'completed'].includes(current?.status)) {
        const source = current?.source_device_id || (connection.flow?.newer === 'target' ? connection.target : connection.source);
        directions = [source === connection.target];
      } else if (!running && [connection.source, connection.target].some(id => data.device_runtime?.[id]?.active_chats?.includes(connection.entry.id))) {
        directions = connection.operation === 'sync' ? [false, true] : [false];
      }
    }
    updateLinkFlow(layer.querySelector('.connection-lines > path'), directions);
  }
  for (const path of document.querySelectorAll('#mobile-links > path')) {
    const phone = data.mobile_devices?.find(device => device.id === path.dataset.id);
    let directions = [];
    if (phone?.active) {
      if (phone.flow_out) directions.push(false);
      if (phone.flow_in) directions.push(true);
    } else if (!phone && online(path.dataset.source) && online(path.dataset.target)) {
      const owner = data.device_runtime?.[path.dataset.source];
      if (chatLinks.some(link => link.operation === 'console' && link.source === path.dataset.source && link.target === path.dataset.target && owner?.active_chats?.includes(link.entry.id))) directions = [false, true];
    }
    updateLinkFlow(path, directions);
  }
}

// One shared group of ports per device; multiple links reuse the same buttons.
let linkFrame = 0, linkForce = false, mapMotionUntil = 0;
function positionConnectionPorts() {
  const allNodes = [...document.querySelectorAll('.map-node')];
  const rectangles = new Map(allNodes.map(node => [node, node.getBoundingClientRect()]));
  const nodes = allNodes.filter(node => node.dataset.kind === 'computer');
  const topology = JSON.stringify([chatLinks.map(({source,target,operation})=>[source,target,operation]), (data.mobile_devices||[]).map(phone=>phone.id)]);
  const geometry = JSON.stringify(allNodes.map(node=>{const r=rectangles.get(node);return [node.dataset.id,r.x,r.y,r.width,r.height];}));
  const now = performance.now(), reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  let moving = false;
  for (const node of nodes) {
    const rect = rectangles.get(node), cx = rect.left + rect.width / 2, cy = rect.top + rect.height / 2;
    const peers = allNodes.filter(other => other !== node);
    if (!peers.length) continue;
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
    const signature = topology + geometry + [...node.querySelectorAll('.connection-port')].map(port=>port.offsetWidth+':'+port.offsetHeight).join(',');
    if (orbit.signature === signature && !orbit.moving) continue;
    orbit.signature = signature;
    orbit.moving = false;
    const wrap = value => (value % orbit.length + orbit.length) % orbit.length;
    const delta = (a, b) => wrap(a - b + orbit.length / 2) - orbit.length / 2;
    const reserved=[],layout=[],channels=[];
    orbit.ports??={};
    const elapsed=1-Math.exp(-Math.min(40,Math.max(1,now-orbit.time))/85);
    for (const [index, operation] of ['sync', 'send', 'receive', 'console'].entries()) {
      const port=node.querySelector('.connection-port.'+operation);
      if(!port)continue;
      const linked=new Set(chatLinks.filter(value=>operation==='console'?value.operation==='console'&&(value.source===node.dataset.id||value.target===node.dataset.id):operation==='sync'?value.operation==='sync'&&(value.source===node.dataset.id||value.target===node.dataset.id):value.operation!=='sync'&&value.operation!=='console'&&(operation==='send'?value.source===node.dataset.id:value.target===node.dataset.id)).map(value=>value.source===node.dataset.id?value.target:value.source));
      if(operation==='console'&&node.dataset.id==='master')for(const phone of data.mobile_devices||[])linked.add(phone.id);
      const targets=peers.filter(peer=>linked.has(peer.dataset.id)).map(peer=>{const r=rectangles.get(peer);return {x:r.left+r.width/2,y:r.top+r.height/2};});
      channels.push({index,operation,port,targets});
    }
    const linkedTargets=channels.flatMap(channel=>channel.targets);
    channels.sort((a,b)=>Number(b.targets.length>0)-Number(a.targets.length>0));
    for(const {index,operation,port,targets}of channels){
      const ideal=orbit.ports[operation]?.target??index*orbit.length/4;
      const width=port.offsetWidth,height=port.offsetHeight;
      const fits=(point,occupied)=>occupied.every(other=>Math.abs(point.x-other.x)>=(width+other.width)/2+6||Math.abs(point.y-other.y)>=(height+other.height)/2+6);
      const candidates=orbit.samples.slice(0,128).map((point,i)=>({point,phase:i*orbit.length/128,distance:targets.length?targets.reduce((sum,peer)=>sum+Math.hypot(cx+point.x-peer.x,cy+point.y-peer.y),0):linkedTargets.length?-linkedTargets.reduce((sum,peer)=>sum+Math.hypot(cx+point.x-peer.x,cy+point.y-peer.y),0):Math.abs(delta(i*orbit.length/128,ideal))}));
      candidates.sort((a,b)=>a.distance-b.distance||Math.abs(delta(a.phase,ideal))-Math.abs(delta(b.phase,ideal)));
      const target=candidates.find(candidate=>fits(candidate.point,reserved));
      if(!target)continue;
      reserved.push({x:target.point.x,y:target.point.y,width,height});
      layout.push({operation,port,target,width,height,fits});
    }
    let phases=layout.map(({operation,target})=>{
      const phase=orbit.ports[operation]?.phase??target.phase;
      return reduced?target.phase:wrap(phase+delta(target.phase,phase)*elapsed);
    });
    const frame=phases.map((phase,index)=>{const point=orbit.path.getPointAtLength(phase);return {x:point.x,y:point.y,width:layout[index].width,height:layout[index].height};});
    if(frame.some((point,index)=>!layout[index].fits(point,frame.slice(0,index))))phases=layout.map(({target})=>target.phase);
    for(const [index,{operation,port,target}]of layout.entries()){
      const state=orbit.ports[operation]??={phase:target.phase,target:target.phase};
      state.target=target.phase;
      const position=phases[index],point=orbit.path.getPointAtLength(position);
      state.phase=position;
      if(Math.abs(delta(state.target,position))>.1){moving=true;orbit.moving=true;}

      const before = orbit.path.getPointAtLength(wrap(position - 0.5)), after = orbit.path.getPointAtLength(wrap(position + 0.5));
      const length = Math.hypot(after.x - before.x, after.y - before.y);
      port.portNormal = { x: (after.y - before.y) / length, y: (before.x - after.x) / length };
      port.dataset.orbit = 'true';
      port.style.left = rect.width / 2 + point.x - node.clientLeft + 'px';
      port.style.top = rect.height / 2 + point.y - node.clientTop + 'px';
      port.querySelector('svg').style.transform = `rotate(${Math.atan2(port.portNormal.y, port.portNormal.x)}rad)`;
    }
    orbit.time=now;orbit.phase=Object.values(orbit.ports).map(port=>port.phase).join(':');
  }
  if (moving) renderLink(true);
}

function renderLink(force = false) {
  linkForce ||= force;
  if (drawingMap || document.hidden || linkFrame) return;
  linkFrame = requestAnimationFrame(() => {
    linkFrame = 0;
    if (document.hidden) return;
    const force = linkForce;
    linkForce = false;
    renderLinkNow(force);
    if (performance.now() < mapMotionUntil) renderLink(true);
  });
}

document.addEventListener('visibilitychange', () => {
  cancelAnimationFrame(linkFrame);
  linkFrame = 0;
  document.documentElement.classList.toggle('map-hidden', document.hidden);
  if (!document.hidden) renderLink(true);
});

function renderLinkNow(force = false) {
  if (drawingMap) return;
  positionConnectionPorts();
  renderMobileLinks();
  if (document.querySelector('.link-badge[data-dragging="true"]')) return;
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
  links.filter(value=>value.operation!=="console").forEach((value, index) => {
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
  renderActiveLinkFlows();
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
  edit.innerHTML = OaclisIcon('edit');
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
  play.innerHTML = OaclisIcon('play');
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
    (operation === "sync" || operation === "receive" || operation === "console");
}

function renderLibrary() {
  const search = $("#search").value.toLocaleLowerCase();
  const focused = document.activeElement?.dataset.chatId;
  const entries = OaclisTerminal.sortChats(visibleChats.filter(
    (c) => !(data.deleted_chats || []).includes(c.id) && (showHiddenChats || !data.chat_settings?.[c.id]?.hidden) && (
      chatLabel(c).toLocaleLowerCase().includes(search) ||
      c.name.toLocaleLowerCase().includes(search) ||
      c.id.toLocaleLowerCase().includes(search)),
  ),chatLabel);
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
      '<span class="library-number">'+esc(data.chat_settings?.[entry.id]?.number??entry.number??'')+'</span>'+badge(entry) +
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
      OaclisIcon('edit');
    edit.onclick = () => editChat(entry, edit);
    row.classList.toggle("hidden-chat", data.chat_settings?.[entry.id]?.hidden === true);
    const remove=document.createElement("button");remove.type="button";
    remove.className="icon-button delete-chat";remove.title=OaclisSettings.text("Elimina definitivamente");
    remove.setAttribute("aria-label",OaclisSettings.text("Elimina definitivamente ")+chatLabel(entry));
    remove.innerHTML=OaclisIcon('trash');
    remove.onclick=guard(()=>deleteChat(entry));
    const run=document.createElement('button');run.type='button';run.className='icon-button library-console';run.title=OaclisSettings.text('Apri console');
    run.setAttribute('aria-label',run.title+' · '+chatLabel(entry));run.innerHTML=OaclisIcon('terminal');
    run.onclick=()=>openChatTerminal({entry},run,row.dataset.sourceDevice);
    row.append(button,run,edit,remove);
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
  const duration = getComputedStyle(document.documentElement).getPropertyValue("--duration-ui").trim();
  const milliseconds = parseFloat(duration) * (duration.endsWith("ms") ? 1 : 1000);
  mapMotionUntil = performance.now() + (matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : milliseconds);
  renderLink(true);
}

function renderMap(animate = false) {
  $("#nodes").connectionLinksState=undefined;
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
      (!mobile && device.id !== "master" && data.ssh_settings?.[device.id]?.in_lan !== true ? OaclisSettings.html(('<span class="remote-badge" role="img" aria-label="Probabilmente remoto" title="Probabilmente remoto">' + OaclisIcon('globe') + '</span>')) : '') + '</span>' +
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
      OaclisIcon('edit');
    customize.onclick = (e) => {
      e.stopPropagation();
      editDevice(device, customize);
    };
    actions.append(customize);
    for (const [action, label, shape] of [
      ["restart", OaclisSettings.text("Riavvia Oaclis"), OaclisIcons.refresh],
      ["toggle-awake", OaclisSettings.text("Sempre attivo"), OaclisIcons.sun],
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
      desktop.innerHTML = OaclisIcon('desktop');
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
      version.innerHTML = OaclisIcon('chevronDown');
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
      access.innerHTML = OaclisIcon('qr');
      access.onclick = event => { event.stopPropagation(); openMobileAccess(access); };
      actions.append(access);
      const remove = document.createElement('button');
      remove.className = 'node-color icon-button'; remove.type = 'button';
      remove.title = OaclisSettings.text('Elimina telefono'); remove.setAttribute('aria-label', OaclisSettings.text('Elimina ') + device.name);
      remove.innerHTML = OaclisIcon('trash');
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
        ["console", OaclisSettings.text("Collega console remota")],
      ]) {
        const port = document.createElement("button");
        port.className = "connection-port " + operation;
        port.dataset.operation = operation;
        port.setAttribute("aria-label", label + " · " + device.name);
        port.title = label;
        port.innerHTML = OaclisIcon(operation === 'sync' ? 'sync' : operation === 'send' ? 'send' : operation === 'console' ? 'square-stop' : 'receive');
        const local = device.id === "master";
  const objectId = device.id || "device-" + crypto.randomUUID();
        if (local) port.title = operation === "send" ? OaclisSettings.text("Invia a un dispositivo") : operation === "receive" ? OaclisSettings.text("Ricevi da un dispositivo") : label;
        const peer = () => !local ? "master" :
          sourceId && sourceId !== "master" ? sourceId : data.devices.length === 1 ? data.devices[0].id : null;
        const origin = () => operation === "receive" || operation === "console" ? peer() : device.id;
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
  renderActiveLinkFlows();
  updateAccessChatActivity();
  const root=$("#nodes");
  const graph=JSON.stringify([chatLinks,link,linkPositions,data.connections,data.appearance,data.chats?.map(chat=>[chat.id,chat.name]),data.deleted_chats,data.mobile_devices?.map(({id,active,chat_ids})=>[id,active,chat_ids])]);
  if(root.connectionLinksState!==graph){root.connectionLinksState=graph;renderLink();}
  document.querySelectorAll(".map-node").forEach((node) => {
    const id = node.dataset.id;
    const mobile = data.mobile_devices?.find(d => d.id === id);
    const state = mobile ? mobile.active ? 'online' : 'offline' :
      id === "master" ? "online" : data.connections[id] || "unknown";
    const runtimeState=data.device_runtime?.[id];
    const visual=JSON.stringify([state,sourceId,deviceName(id),data.appearance?.['device:'+id]?.color,runtimeState?.available,runtimeState?.active,runtimeState?.error,runtimeState?.desktop?.id,runtimeState?.build,runtimeState?.update?.status,data.build?.source_id,[...node.querySelectorAll('button')].map(button=>[button.dataset.busy,button.dataset.checking])]);
    if(node.connectionVisual===visual)return;
    node.connectionVisual=visual;
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
    scopes: data.chat_settings?.[entry.id]?.scopes || { chat: true, paths: true, apps: true }, transfer_pending: operation!=="console" };
  try {
    await saveLinkChange(async () => {
      chatLinks = await api("links/save", association);
      link = null;
      renderLink(true);
    });
  } catch (error) { notice(OaclisSettings.text("Associazione non salvata: ") + error.message); }
}


function mobileLinkCurve(from, to, base, radius = 0, fromPort = null, toPort = null) {
  const overlapX = [Math.max(from.left + radius, to.left + radius), Math.min(from.right - radius, to.right - radius)];
  const overlapY = [Math.max(from.top + radius, to.top + radius), Math.min(from.bottom - radius, to.bottom - radius)];
  const sides = [from, to].map((rect, index) => {
    const port=index?toPort:fromPort;
    if(port){const r=port.getBoundingClientRect(),n=port.portNormal||{x:1,y:0};return [{x:r.left+r.width/2,y:r.top+r.height/2,nx:n.x,ny:n.y}];}
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
  const groups = (data.mobile_devices || []).map(phone => ({...phone, source:'master', target:phone.id,
    entries:(phone.chat_ids || []).filter(id=>!(data.deleted_chats || []).includes(id)).map(id => data.chats.find(chat=>chat.id===id) || {id,name:id})}));
  for (const connection of chatLinks.filter(link=>link.operation==='console' && !(data.deleted_chats || []).includes(link.entry.id))) {
    const id = 'console-' + connection.source + '-' + connection.target;
    let group = groups.find(group=>group.id===id);
    if (!group) { group={id,source:connection.source,target:connection.target,active:data.connections[connection.source==='master'?connection.target:connection.source]==='online',entries:[]}; groups.push(group); }
    group.entries.push(connection.entry);
  }
  const nodes=new Map([...document.querySelectorAll('.map-node')].map(node=>[node.dataset.id,node]));
  const masterNode=nodes.get('master');
  if (!groups.length || !masterNode) { svg?.remove(); document.querySelectorAll('.access-chat').forEach(node=>{node.accessPanel?.remove();node.remove();}); return; }
  if (!svg) {
    svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.id = 'mobile-links'; svg.setAttribute('aria-hidden', 'true');
    svg.innerHTML = '<defs></defs>';
    $('#nodes').prepend(svg);
  }
  const base = $('#nodes').getBoundingClientRect();
  const radius = parseFloat(getComputedStyle(masterNode).borderTopLeftRadius) || 0;
  const keep = new Set();
  const badges = new Set();
  const markers=new Map([...document.querySelectorAll('.access-chat')].map(marker=>[marker.dataset.accessKey,marker]));
  const paths=new Map([...svg.querySelectorAll(':scope > path')].map(path=>[path.dataset.id,path]));
  const arrows=new Map([...svg.querySelector('defs').children].map(marker=>[marker.dataset.id,marker]));
  for (const device of groups) {
    const node=nodes.get(device.target),owner=nodes.get(device.source);
    if (!node || !owner) continue;
    keep.add(device.id);
    let path=paths.get(device.id);
    if (!path) {
      path = document.createElementNS(svg.namespaceURI, 'path'); path.dataset.id = device.id;
      const marker = document.createElementNS(svg.namespaceURI, 'marker'); marker.dataset.id = device.id;
      marker.id = 'mobile-arrow-' + device.id; marker.setAttribute('viewBox','0 0 10 10'); marker.setAttribute('refX','9'); marker.setAttribute('refY','5');
      marker.setAttribute('markerWidth','10'); marker.setAttribute('markerHeight','10'); marker.setAttribute('markerUnits','userSpaceOnUse'); marker.setAttribute('orient','auto');
      marker.innerHTML = '<path d="M1 1L9 5L1 9Z"/>'; svg.querySelector('defs').append(marker);
      path.setAttribute('marker-end','url(#' + marker.id + ')'); svg.append(path);arrows.set(device.id,marker);
    }
    path.dataset.source=device.source; path.dataset.target=device.target;
    const from=owner.getBoundingClientRect(),to=node.getBoundingClientRect(),ports=[owner.querySelector('.connection-port.console'),node.querySelector('.connection-port.console')];
    const geometry=JSON.stringify([base.x,base.y,radius,...[from,to].map(r=>[r.x,r.y,r.width,r.height]),...ports.map(port=>{const r=port?.getBoundingClientRect();return r?[r.x,r.y,r.width,r.height,port.portNormal]:null;})]);
    if(path.dataset.geometry!==geometry){path.accessCurve=mobileLinkCurve(from,to,base,radius,...ports);path.setAttribute('d',path.accessCurve);path.dataset.geometry=geometry;}
    renderAccessBadges(path, device, badges, markers);
    svg.querySelectorAll('.link-flow-trace[data-id="' + CSS.escape(device.id) + '"]').forEach(trace => trace.setAttribute('d', path.getAttribute('d')));
    path.dataset.active = String(device.active === true);
    arrows.get(device.id).dataset.active = path.dataset.active;
  }
  document.querySelectorAll('.access-chat').forEach(node=>{if(!badges.has(node.dataset.accessKey)){node.accessPanel?.remove();node.remove();}});
  for (const item of [...svg.querySelectorAll(':scope > path, defs > marker')]) if (!keep.has(item.dataset.id)) { item.flowOverlay?.remove(); item.remove(); }
  updateAccessChatActivity();
}

function openAccessChats(marker) {
  if(marker.accessPanel?.isConnected){marker.accessPanel.querySelector('.popover-close').focus();return;}
  const {group}=marker.accessState,panel=document.createElement('div');
  panel.className='access-chat-list';panel.setAttribute('popover','manual');panel.setAttribute('role','dialog');
  panel.setAttribute('aria-label',group.name||deviceName(group.target));
  panel.innerHTML='<button type="button" class="icon-button popover-close" aria-label="'+esc(OaclisSettings.text('Chiudi'))+'">'+OaclisIcon('close')+'</button><h2>'+esc(group.name||deviceName(group.target))+'</h2>';
  for(const entry of OaclisTerminal.sortChats(group.entries,chatLabel)){
    const row=document.createElement('div');row.className='access-chat-row';row.dataset.chatId=entry.id;
    row.innerHTML=badge(entry)+'<span class="access-chat-name">'+esc(chatLabel(entry))+'</span><button type="button" class="icon-button">'+OaclisIcon('terminal')+'</button>';
    row.querySelector('.chat-badge').style.color=appearance(entry).color||'var(--ink)';
    const open=row.querySelector('button');open.title=OaclisSettings.text('Apri console')+' · '+chatLabel(entry);open.setAttribute('aria-label',open.title);
    open.onclick=()=>{panel.hidePopover();openChatTerminal({entry},marker.querySelector('.link-main'),group.source);};
    panel.append(row);
  }
  panel.querySelector('.popover-close').onclick=()=>panel.hidePopover();
  panel.addEventListener('toggle',event=>{if(event.newState==='closed'){panel.remove();marker.querySelector('.link-main').setAttribute('aria-expanded','false');}});
  marker.accessPanel=panel;document.body.append(panel);panel.showPopover();positionContext(panel,marker);
  marker.querySelector('.link-main').setAttribute('aria-expanded','true');
  updateAccessChatActivity();
}

function updateAccessChatActivity() {
  for (const marker of document.querySelectorAll('.access-chat')) {
    const {group,grouped,entry}=marker.accessState,active=data.device_runtime?.[group.source]?.active_chats || [];
    const value=String(grouped?group.entries.some(chat=>active.includes(chat.id)):active.includes(entry.id));
    if(marker.dataset.sessionOpen!==value)marker.dataset.sessionOpen=value;
    for(const row of marker.accessPanel?.querySelectorAll('.access-chat-row') || []){
      const value=String(active.includes(row.dataset.chatId));
      if(row.dataset.sessionOpen!==value)row.dataset.sessionOpen=value;
    }
  }
}

function renderAccessBadges(path, group, keep, markers) {
  if(path.accessCurve&&path.getAttribute('d')!==path.accessCurve)path.setAttribute('d',path.accessCurve);
  const style=getComputedStyle($('#nodes')),length=path.getTotalLength(),count=group.entries.length;
  const spacing=parseFloat(style.getPropertyValue('--link-badge-size'))+parseFloat(style.getPropertyValue('--space-18'))+parseFloat(style.getPropertyValue('--space-12'));
  const points=count>1&&length>=spacing*(count+1)?group.entries.map((_,index)=>path.getPointAtLength(length*(index+1)/(count+1))):[];
  const grouped=count>1&&(points.length===0||points.some((point,index)=>points.slice(0,index).some(other=>Math.abs(point.x-other.x)<spacing&&Math.abs(point.y-other.y)<spacing)));
  const entries=grouped?[{id:'access-group',name:group.name||deviceName(group.target)}]:OaclisTerminal.sortChats(group.entries,chatLabel);
  entries.forEach((entry,index) => {
    const key=[entry.id,...[group.source,group.target].sort()].join(':');
    keep.add(key);
    let marker=markers.get(key);
    if (!marker) {
      marker=document.createElement('div'); marker.className='link-badge access-chat'; marker.dataset.accessKey=key;
      marker.innerHTML='<button type="button" class="link-main"></button><button type="button" class="link-open icon-button">'+OaclisIcon('terminal')+'</button>';
      $('#nodes').append(marker);markers.set(key,marker);
      const main=marker.querySelector('.link-main'),open=marker.querySelector('.link-open');
      marker.updateAccessCurve=()=>{
        if(!marker.accessBend)return;
        path.setAttribute('d',marker.accessBend({x:parseFloat(marker.style.left),y:parseFloat(marker.style.top)}));
        path.flowOverlay?.querySelectorAll('.link-flow-trace').forEach(trace=>trace.setAttribute('d',path.getAttribute('d')));
      };
      open.onclick=()=>openChatTerminal({entry:marker.accessState.entry},open,marker.accessState.source);
      main.onclick=event=>{if(marker.accessState.grouped&&(event.detail===0||!marker.accessMoved))openAccessChats(marker);};
      const save=()=>{
        const {entry,source,target,midpoint}=marker.accessState;
        linkPositions[key]={x:marker.offsetLeft-midpoint.x,y:marker.offsetTop-midpoint.y};linkPositionSaving=true;
        api('link-position',{id:entry.id,source,target,...linkPositions[key]})
          .catch(error=>notice(error.message)).finally(()=>{linkPositionSaving=false;});
      };
      main.onpointerdown=event=>{
        if(event.button!==0)return;
        event.preventDefault();marker.accessMoved=false;const start={x:event.clientX,y:event.clientY,left:marker.offsetLeft,top:marker.offsetTop};
        marker.dataset.dragging='true';main.setPointerCapture(event.pointerId);
        main.onpointermove=event=>{
          if(!main.hasPointerCapture(event.pointerId))return;
          if(Math.hypot(event.clientX-start.x,event.clientY-start.y)>4)marker.accessMoved=true;
          marker.style.left=Math.max(22,Math.min($('#nodes').clientWidth-22,start.left+event.clientX-start.x))+'px';
          marker.style.top=Math.max(22,Math.min($('#nodes').clientHeight-22,start.top+event.clientY-start.y))+'px';
          marker.updateAccessCurve();
        };
        main.onpointerup=event=>{if(marker.accessMoved)save();delete marker.dataset.dragging;if(main.hasPointerCapture(event.pointerId))main.releasePointerCapture(event.pointerId);};
        main.onpointercancel=main.onlostpointercapture=()=>{delete marker.dataset.dragging;};
      };
      main.onkeydown=event=>{
        const direction={ArrowLeft:[-10,0],ArrowRight:[10,0],ArrowUp:[0,-10],ArrowDown:[0,10]}[event.key];
        if(direction){event.preventDefault();marker.style.left=marker.offsetLeft+direction[0]+'px';marker.style.top=marker.offsetTop+direction[1]+'px';marker.updateAccessCurve();save();}
        if(event.key==='Delete'&&!marker.accessState.grouped&&marker.accessState.console){const {entry,source,target}=marker.accessState;guard(()=>removeConnection({entry,source,target,operation:'console'},null))();}
      };
    }
    if(marker.dataset.dragging)return;
    const main=marker.querySelector('.link-main'),open=marker.querySelector('.link-open');
    open.hidden=grouped;
    const html=grouped?'<span class="chat-badge">'+group.entries.length+'</span>':badge(entry),name=grouped?entry.name+' · '+group.entries.length:chatLabel(entry),color=grouped?'var(--ink)':appearance(entry).color||'var(--ink)';
    const presentation=JSON.stringify([html,name,color,grouped?group.entries.map(entry=>[entry.id,chatLabel(entry),appearance(entry)]):null]);
    if(marker.dataset.presentation!==presentation){
      marker.accessPanel?.remove();
      main.innerHTML=html;marker.title=name;main.title=(grouped?OaclisSettings.text('Mostra chat')+' · ':OaclisSettings.text('Trascina '))+name;
      if(grouped){main.setAttribute('aria-haspopup','dialog');main.setAttribute('aria-expanded','false');}else{main.removeAttribute('aria-haspopup');main.removeAttribute('aria-expanded');}
      main.setAttribute('aria-label',main.title);open.title=OaclisSettings.text('Apri console')+' · '+name;open.setAttribute('aria-label',open.title);
      marker.style.color=color;main.querySelector('.chat-badge').style.color=color;marker.dataset.presentation=presentation;
    }
    const geometry = JSON.stringify([path.getAttribute('d'),index,entries.length]);
    if (marker.dataset.geometry !== geometry) {
      marker.accessMidpoint = path.getPointAtLength(path.getTotalLength()*(index+1)/(entries.length+1));
      const coordinates=path.getAttribute('d').match(/-?\d*\.?\d+(?:e[-+]?\d+)?/gi).map(Number);
      marker.accessBend=entries.length===1?curveThroughBadge(Array.from({length:4},(_,i)=>({x:coordinates[i*2],y:coordinates[i*2+1]})),marker.accessMidpoint):null;
      marker.dataset.geometry = geometry;
    }
    const midpoint = marker.accessMidpoint;
    marker.accessState={entry,group,grouped,source:group.source,target:group.target,console:group.id.startsWith('console-'),midpoint};
    const offset=linkPositions[key]||{x:0,y:0};
    marker.style.left=midpoint.x+offset.x+'px';marker.style.top=midpoint.y+offset.y+'px';
    marker.updateAccessCurve();
  });
}
