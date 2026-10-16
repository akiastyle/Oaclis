// app.js v3.160.2924
"use strict";
const $ = (s) => document.querySelector(s);
const token = $('meta[name="oaclis-token"]').content;
const esc = (v) =>
  String(v ?? "").replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );
let data = {
    devices: [],
    chats: [],
    ssh_settings: {},
    connections: {},
    job: { status: "idle" },
  },
  sourceId = null,
  selected = null,
  link = null,
  visibleChats = [],
  drag = null,
  positions = {},
  linkPositions = {},
  linkPositionSaving = false,
  catalogRequest = 0,
  audio,
  noticeTimer;
try {
  positions = JSON.parse(localStorage.getItem("oaclis-map") || "{}");
} catch {
  positions = {};
}
const catalogFailures = new Set();
let chatLinks = [];
let saveLinks = Promise.resolve();
function saveLinkChange(action) {
  const pending = saveLinks.then(action);
  saveLinks = pending.catch(() => {});
  return pending;
}
let removingPresentation = false;
let drawingMap = false;
const master = { id: "master", name: "Questo dispositivo" };
const devices = () => [{ ...master, name: data.appearance?.["device:master"]?.label || master.name }, ...data.devices];
const deviceName = (id) => devices().find((d) => d.id === id)?.name || id;
document.documentElement.dataset.theme =
  localStorage.getItem("oaclis-theme") ||
  (matchMedia("(prefers-color-scheme:dark)").matches ? "dark" : "light");
$("#theme").onclick = () => {
  const theme =
    document.documentElement.dataset.theme === "dark" ? "light" : "dark";
  document.documentElement.dataset.theme = theme;
  localStorage.setItem("oaclis-theme", theme);
};
const palettes = {
  sienna: ["#3d5a80", "#98c1d9", "#e0fbfc", "#ee6c4d", "#293241"],
  imperial: ["#2b2d42", "#8d99ae", "#edf2f4", "#ef233c", "#d90429"],
  honey: ["#8ecae6", "#219ebc", "#023047", "#ffb703", "#fb8500"],
  rose: ["#355070", "#6d597a", "#b56576", "#e56b6f", "#eaac8b"],
  cyber: ["#00296b", "#003f88", "#00509d", "#fdc500", "#ffd500"],
  paradise: ["#ef476f", "#ffd166", "#06d6a0", "#118ab2", "#073b4c"],
  lemon: ["#003049", "#d62828", "#f77f00", "#fcbf49", "#eae2b7"],
  oxford: ["#000000", "#14213d", "#fca311", "#e5e5e5", "#ffffff"],
  carolina: ["#ffffff", "#00171f", "#003459", "#007ea7", "#00a8e8"],
  blue: ["#03045e", "#0077b6", "#00b4d8", "#90e0ef", "#caf0f8"],
  midnight: ["#177e89", "#084c61", "#db3a34", "#ffc857", "#323031"],
  sage: ["#033f63", "#28666e", "#7c9885", "#b5b682", "#fedc97"],
  almond: ["#f1dac4", "#a69cac", "#474973", "#161b33", "#0d0c1d"],
  ruby: ["#d81159", "#8f2d56", "#218380", "#fbb13c", "#73d2de"],
  atomic: ["#f79256", "#fbd1a2", "#7dcfb6", "#00b2ca", "#1d4e89"],
  cadet: ["#dd6e42", "#e8dab2", "#4f6d7a", "#c0d6df", "#eaeaea"],
  jet: ["#353535", "#3c6e71", "#ffffff", "#d9d9d9", "#284b63"],
  sunglow: ["#ff595e", "#ffca3a", "#8ac926", "#1982c4", "#6a4c93"],
  salmon: ["#70d6ff", "#ff70a6", "#ff9770", "#ffd670", "#e9ff70"],
  coffee: ["#3c3744", "#090c9b", "#3066be", "#b4c5e4", "#fbfff1"],
  cerise: ["#0a2463", "#3e92cc", "#fffaff", "#d8315b", "#1e1b18"],
};
let palette = localStorage.getItem("oaclis-palette") || "sienna";
if (!palettes[palette]) palette = "sienna";
function automaticColor(id) {
  // Stable pseudo-random assignment: polling never changes a chat's color.
  let hash = 0;
  for (const c of id) hash = (Math.imul(hash, 31) + c.charCodeAt(0)) | 0;
  return palettes[palette][(hash >>> 0) % palettes[palette].length];
}
function paletteSwatches(colors) {
  const preview = document.createElement("span");
  preview.className = "palette-swatches";
  preview.setAttribute("aria-hidden", "true");
  for (const color of colors) {
    const swatch = document.createElement("i");
    swatch.style.backgroundColor = color;
    preview.append(swatch);
  }
  return preview;
}
function updatePalettePreview() {
  document.documentElement.dataset.palette = palette;
  document.querySelectorAll(".palette-option").forEach(button => {
    const active = button.dataset.palette === palette;
    button.setAttribute("aria-checked", String(active));
    button.tabIndex = active ? 0 : -1;
    if (active) $("#palette").title = button.textContent.trim();
  });
}
const paletteOptions = [...document.querySelectorAll(".palette-option")];
for (const button of paletteOptions) {
  button.append(paletteSwatches(palettes[button.dataset.palette]));
  button.onclick = () => {
    palette = button.dataset.palette;
    localStorage.setItem("oaclis-palette", palette);
    updatePalettePreview();
    renderMap();
    renderLibrary();
    $("#palette-menu").hidePopover();
    $("#palette").focus();
  };
}
$("#palette-menu").addEventListener("toggle", e => {
  if (e.newState !== "open") return;
  const anchor = $("#palette").getBoundingClientRect();
  const menu = e.target;
  menu.style.left = Math.max(8, Math.min(anchor.left, innerWidth - menu.offsetWidth - 8)) + "px";
  menu.style.top = Math.max(8, Math.min($(".toolbar").getBoundingClientRect().bottom + 8, innerHeight - menu.offsetHeight - 8)) + "px";
  menu.querySelector('[aria-checked="true"]').focus({ preventScroll: true });
});
$("#palette-menu").onkeydown = e => {
  const index = paletteOptions.indexOf(document.activeElement);
  let next;
  if (e.key === "ArrowDown" || e.key === "ArrowRight") next = (index + 1) % paletteOptions.length;
  if (e.key === "ArrowUp" || e.key === "ArrowLeft") next = (index - 1 + paletteOptions.length) % paletteOptions.length;
  if (e.key === "Home") next = 0;
  if (e.key === "End") next = paletteOptions.length - 1;
  if (next !== undefined) { e.preventDefault(); paletteOptions[next].focus(); }
};
updatePalettePreview();
$("#sound").onclick = () => {
  const enabled = $("#sound").getAttribute("aria-pressed") !== "true";
  $("#sound").setAttribute("aria-pressed", enabled);
  if (enabled) {
    audio ??= new AudioContext();
    audio.resume();
  }
};
function beep() {
  if (!audio || $("#sound").getAttribute("aria-pressed") !== "true") return;
  const osc = audio.createOscillator(),
    gain = audio.createGain();
  osc.connect(gain);
  gain.connect(audio.destination);
  osc.frequency.value = 660;
  gain.gain.setValueAtTime(0.08, audio.currentTime);
  gain.gain.exponentialRampToValueAtTime(0.001, audio.currentTime + 0.3);
  osc.start();
  osc.stop(audio.currentTime + 0.3);
}
async function api(path, value) {
  const response = await fetch("/api/" + path, {
    method: value === undefined ? "GET" : "POST",
    headers: { "X-Oaclis-Token": token, "Content-Type": "application/json" },
    body: value === undefined ? undefined : JSON.stringify(value),
  });
  if (response.status === 403) {
    location.reload();
    throw Error("Sessione scaduta.");
  }
  const result = await response.json();
  if (!response.ok) throw Error(result.error || "Operazione non riuscita");
  return result;
}
function notice(text) {
  clearTimeout(noticeTimer);
  $("#notice").textContent = text;
  $("#notice").hidden = false;
  noticeTimer = setTimeout(() => ($("#notice").hidden = true), 6000);
}
function guard(fn) {
  return async (...args) => {
    try {
      await fn(...args);
    } catch (e) {
      notice(e.message);
    }
  };
}
const eyePath = '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/>';
function renderEye(button, crossed, label) {
  button.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true">' + eyePath + (crossed ? '<path d="m3 3 18 18"/>' : '') + '</svg>';
  button.title = label; button.setAttribute("aria-label", label);
  button.setAttribute("aria-pressed", String(!crossed));
}
let showHiddenChats = false;
renderEye($("#show-hidden"), true, "Mostra chat nascoste");
$("#show-hidden").onclick = () => {
  showHiddenChats = !showHiddenChats;
  renderEye($("#show-hidden"), !showHiddenChats, showHiddenChats ? "Nascondi chat nascoste" : "Mostra chat nascoste");
  renderLibrary();
};
$("#close").onclick = closePanel;
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && $("#path-authorization[open]")) return;
  if (e.key === "Escape" && $("#terminal-choice")) { $("#terminal-choice .popover-close").click(); return; }
  if (e.key === "Escape" && document.activeElement?.closest(".chat-terminal")) { closeChatTerminal(document.activeElement.closest(".chat-terminal")); return; }
  if (e.key === "Escape" && $("#device-settings")) { closeDeviceSettings(); return; }
  if (e.key === "Escape" && $("#connection-popover")) { closeConnectionPopover(); return; }
  if (e.key === "Escape" && $(".chat-editor")) { closeChatEditor(); return; }
  if (e.key === "Escape" && !$("#panel").hidden) closePanel();
});
$("#back").onclick = showLibrary;
async function openDevice(id) {
  sourceId = id;
  $("#panel-device").textContent = deviceName(id);
  $("#search").value = "";
  visibleChats = [];
  showLibrary();
  openPanel();
  $("#panel").opener = [...document.querySelectorAll(".map-node")].find(node => node.dataset.id === id);
  bindPanelDrag($("#panel"), true);
  renderConnections();
  await loadChats();
  if (sourceId === id && !$("#panel").hidden) $("#search").focus();
}
async function loadChats() {
  if (!sourceId) return;
  const request = ++catalogRequest;
  const id = sourceId;
  catalogFailures.delete(id);
  if (id !== "master" && data.connections[id] === "offline") {
    $("#library-list").innerHTML = '<p class="muted">Dispositivo offline.</p>';
    return;
  }
  $("#library-list").innerHTML = '<p class="muted">Caricamento…</p>';
  try {
    const result = await api("catalog", { device_id: id });
    if (request !== catalogRequest || sourceId !== id) return;
    visibleChats = result.chats;
    if (id === "master") data.codex_root = result.root;
    renderLibrary();
  } catch (e) {
    if (request !== catalogRequest) return;
    catalogFailures.add(id);
    $("#library-list").innerHTML =
      '<p class="muted">' + esc(e.message) + "</p>";
    throw e;
  }
}
$("#refresh").onclick = guard(loadChats);
$("#search").oninput = renderLibrary;
const appearance = (entry) => {
  const saved = data.appearance?.["chat:" + entry.id] || {};
  return { ...saved, color: saved.color || automaticColor(entry.id) };
};
const chatLabel = (entry) => appearance(entry).label || entry.name;
const icons = {
  chat: '<path d="M4 4h16v12H9l-5 4z"/>',
  code: '<path d="m8 6-6 6 6 6m8-12 6 6-6 6m-2-16-4 20"/>',
  folder: '<path d="M3 6h7l2 3h9v11H3z"/>',
  desktop:
    '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8m-4-4v4"/>',
  laptop: '<path d="M5 4h14v13H5zm-3 16h20l-3-3H5z"/>',
  server:
    '<rect x="4" y="3" width="16" height="18" rx="2"/><path d="M4 9h16M4 15h16M8 6h1M8 12h1M8 18h1"/>',
  terminal: '<path d="m4 6 6 6-6 6m9 0h7"/>',
  globe: '<circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="4" ry="9"/><path d="M3 12h18"/>',
  cloud: '<path d="M6 18a5 5 0 1 1 1-10 6 6 0 0 1 11 2 4 4 0 0 1 0 8z"/>',
  box: '<path d="m3 7 9-4 9 4v10l-9 4-9-4zm0 0 9 5 9-5m-9 5v9"/>',
  database: '<ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14c0 4 16 4 16 0V5M4 12c0 4 16 4 16 0"/>',
  git: '<circle cx="7" cy="5" r="2"/><circle cx="7" cy="19" r="2"/><circle cx="17" cy="7" r="2"/><path d="M7 7v10m0-4h5a5 5 0 0 0 5-4"/>',
  book: '<path d="M12 5c-3-2-6-2-10-1v15c4-1 7-1 10 1 3-2 6-2 10-1V4c-4-1-7-1-10 1zm0 0v15"/>',
  star: '<path d="m12 3 3 6 7 1-5 5 1 7-6-3-6 3 1-7-5-5 7-1z"/>',
  bolt: '<path d="M13 2 4 14h7l-1 8 10-13h-7z"/>',
  shield: '<path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6z"/>',
  bug: '<rect x="7" y="7" width="10" height="14" rx="5"/><path d="m9 7-2-4m8 4 2-4M3 10h4m10 0h4M3 15h4m10 0h4M5 21l3-3m8 0 3 3M12 7v14"/>',
  settings: '<path d="m9 3-1 3-3 1-2 3 2 2-1 3 3 2 3-1 2 3 3-1 1-3 3-1 1-3-2-2 1-3-3-2-3 1-2-3z"/><circle cx="12" cy="12" r="3"/>',
  cpu: '<rect x="6" y="6" width="12" height="12" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M9 2v4m6-4v4M9 18v4m6-4v4M2 9h4m-4 6h4m12-6h4m-4 6h4"/>',
  rocket: '<path d="M9 15c-3-5 0-11 12-12 0 12-6 15-11 12zm0-7L4 9l-2 5 6-1m8 2-1 5-5 2 1-6M7 17l-4 4"/><circle cx="16" cy="8" r="2"/>',
  link: '<path d="m10 13 4-4m-6 6-2 2a4 4 0 0 1-6-6l4-4a4 4 0 0 1 6 0m0 10a4 4 0 0 0 6 0l4-4a4 4 0 0 0-6-6l-2 2"/>',
  image: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8" cy="8" r="2"/><path d="m3 17 5-5 4 4 4-6 5 7"/>',
  wrench: '<path d="M14 4a6 6 0 0 0-7 7l-5 5a3 3 0 0 0 4 4l5-5a6 6 0 0 0 7-7l-4 4-3-3z"/>',
  layers: '<path d="m12 3 10 5-10 5L2 8zm-10 9 10 5 10-5M2 16l10 5 10-5"/>',
};
const iconNames = { chat: "Chat", code: "Codice", folder: "Cartella", desktop: "Desktop", laptop: "Portatile", server: "Server", terminal: "Terminale", globe: "Globo", cloud: "Cloud", box: "App", database: "Database", git: "Rami", book: "Libro", star: "Stella", bolt: "Fulmine", shield: "Scudo", bug: "Bug", settings: "Impostazioni", cpu: "Processore", rocket: "Razzo", link: "Collegamento", image: "Immagine", wrench: "Strumenti", layers: "Livelli" };
function iconBody(icon, name) {
  if (icon?.startsWith("data:image/png;base64,")) return '<img src="' + esc(icon) + '" alt="">';
  if (icons[icon]) return '<svg viewBox="0 0 24 24" aria-hidden="true">' + icons[icon] + '</svg>';
  return esc([...name.trim()][0]?.toLocaleUpperCase() || "?");
}
function badge(entry) {
  return '<span class="chat-badge" aria-hidden="true">' + iconBody(appearance(entry).icon, chatLabel(entry)) + '</span>';
}
let positionTimer, positionsSaving = false;
let mapMotionFrame;
$("#reset-map").onclick = () => {
  positions = {};
  savePositions();
  renderMap();
};
$("#check").onclick = guard(() => api("check", {}));
$("#add-device").onclick = () => openDiscovery();
$("#edit-device").onclick = () => editDevice(data.devices.find(d => d.id === sourceId), $("#edit-device"));
const lastConnection = {};
async function poll() {
  const saved = saveLinks;
  await saved;
  if (saved !== saveLinks) return;
  const next = await api("status");
  for (const device of data.devices) {
    const state = next.connections[device.id];
    if (state === "online" && lastConnection[device.id] === "offline") {
      catalogFailures.delete(device.id);
      notice(device.name + " raggiungibile");
      beep();
    }
    if (state !== "checking") lastConnection[device.id] = state;
  }
  if (next.shared && saved === saveLinks && !removingPresentation && !positionsSaving && !linkPositionSaving && !drag && !document.querySelector('.map-node[data-dragging="true"], .link-badge[data-dragging="true"]')) {
    const previous = JSON.stringify([data.devices, data.appearance, chatLinks, data.chat_settings, positions, linkPositions]);
    const shared = next.shared;
    if (previous !== JSON.stringify([shared.devices, shared.appearance, shared.links, shared.chat_settings, shared.positions, shared.link_positions])) {
      const nodesChanged = JSON.stringify([data.devices, Object.fromEntries(Object.entries(data.appearance || {}).filter(([key]) => key.startsWith("device:"))), positions]) !==
        JSON.stringify([shared.devices, Object.fromEntries(Object.entries(shared.appearance || {}).filter(([key]) => key.startsWith("device:"))), shared.positions]);
      const libraryChanged = JSON.stringify([data.appearance, data.chat_settings]) !== JSON.stringify([shared.appearance, shared.chat_settings]);
      Object.assign(data, shared); chatLinks = shared.links; positions = shared.positions; linkPositions = shared.link_positions || {};
      if (nodesChanged) renderMap(true);
      if (nodesChanged || libraryChanged) renderLibrary();
    }
  }
  data.device_runtime ??= {};
  if (next.device_runtime) for (const id of Object.keys(data.device_runtime)) if (!(id in next.device_runtime)) delete data.device_runtime[id];
  for (const [id, runtime] of Object.entries(next.device_runtime || {})) {
    const previous = data.device_runtime[id];
    if (previous?.update?.status === "running" && runtime.update?.status === "failed") { console.error("Oaclis update:", runtime.update.error); notice("Aggiornamento non riuscito"); }
    if (runtime.available === false || previous?.available === false || !previous?.observed_at || !runtime.observed_at || runtime.observed_at >= previous.observed_at) data.device_runtime[id] = runtime;
  }
  data.build = next.build;
  data.connections = next.connections;
  data.job = next.job;
  renderPathAuthorization(next.job?.authorization);
  data.paused = next.paused;
  data.checks_paused = next.checks_paused;
  data.settings_paused = next.settings_paused;
  data.analyses = next.analyses || {};
  data.app_checks = next.app_checks || {};
  data.transfers = next.transfers || {};
  data.preparations = next.preparations || {};
  for (const editor of document.querySelectorAll(".chat-editor")) {
    if (editor.scopeEntry) { renderScopeDetails(editor, editor.scopeEntry); editor.updateVisibility?.(); }
  }
  if (next.discovery) await updateDiscovery(next.discovery);
  renderConnections();
  for (const marker of document.querySelectorAll(".link-badge")) {
    const [id, source] = JSON.parse(marker.dataset.linkKey);
    const connection = chatLinks.find(value => value.entry.id === id && value.source === source && value.target === JSON.parse(marker.dataset.linkKey)[2]);
    if (connection) updateBadgeMotion(marker, connection);
  }
}
let polling = false;
let lastPoll = 0;
setInterval(async () => {
  const transferring = data.job?.status === "running" || document.querySelector('.link-badge[data-motion-phase="transferring"]');
  if (polling || Date.now() - lastPoll < (transferring ? 500 : 2000)) return;
  lastPoll = Date.now();
  polling = true;
  try {
    await poll();
  } catch (e) {
    notice(e.message);
  } finally {
    polling = false;
  }
}, 500);

let refreshing = false;
setInterval(async () => {
  if (refreshing || !sourceId || $("#panel").hidden || drag || catalogFailures.has(sourceId) ||
      (sourceId !== "master" && data.connections[sourceId] !== "online")) return;
  refreshing = true;
  const id = sourceId,
    version = catalogRequest;
  try {
    const result = await api("catalog", { device_id: id });
    if (sourceId !== id || version !== catalogRequest) return;
    for (const editor of document.querySelectorAll(".chat-editor")) {
      if (editor.dataset.sourceDevice !== id) continue;
      const entry = result.chats.find(c => c.id === editor.dataset.objectId);
      if (entry) renderScopeDetails(editor, entry);
    }
    if (JSON.stringify(result.chats) !== JSON.stringify(visibleChats) || visibleChats.length === 0) {
      visibleChats = result.chats;
      renderLibrary();
      if (selected) {
        const next = visibleChats.find((c) => c.id === selected.id);
        if (next) {
          selected = next;
          const heading = $("#details h2");
          if ($("#chat-form") && heading) heading.textContent = chatLabel(next);
        }
      }
    }
  } catch (e) {
    if (sourceId === id && version === catalogRequest) { catalogFailures.add(id); notice(e.message); }
  } finally {
    refreshing = false;
  }
}, 10000);
window.onresize = () => {
  renderMap();
  for (const panel of document.querySelectorAll('.context-popover:popover-open')) positionContext(panel, panel.opener);
};
guard(async () => {
  data = await api("state");
  chatLinks = data.links || [];
  const oldPositions = positions;
  positions = data.positions || {};
  linkPositions = data.link_positions || {};
  renderMap();
  if (data.role === "master" && !Object.keys(positions).length) {
    positions = Object.fromEntries([...document.querySelectorAll(".map-node")].map(n => [n.dataset.id, oldPositions[n.dataset.id] || { x: n.offsetLeft, y: n.offsetTop }]));
    await api("positions", positions);
    localStorage.removeItem("oaclis-map");
    renderMap();
  }
  await updateDiscovery(await api("peers"), true);
  await api("check", {});
})();

let discovery = { peers: [] };
const noticedPeers = new Set();
