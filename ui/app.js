// app.js v2.89.1662
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
$("#close").onclick = closePanel;
document.addEventListener("keydown", (e) => {
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
  const checking = scope !== "chat" && analysis.status === "pending" || (scope === "apps" || scope === "all") && data.app_checks?.[key]?.status === "pending";
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
function requestAppCheck(panel, entry) {
  const source = panel.dataset.sourceDevice || "master", key = source + ":" + entry.id;
  if (panel.appCheckInFlight || data.app_checks?.[key]) return;
  panel.appCheckInFlight = true;
  api("app-check", { source_device_id: source, chat_id: entry.id }).then(result => {
    data.app_checks ??= {}; data.app_checks[key] = result;
    if (panel.isConnected) renderScopeDetails(panel, panel.scopeEntry);
  }).catch(error => {
    data.app_checks ??= {}; data.app_checks[key] = { status: "failed", error: error.message };
    if (panel.isConnected) renderScopeDetails(panel, panel.scopeEntry);
  }).finally(() => { panel.appCheckInFlight = false; });
}
function renderScopeDetails(panel, entry) {
  panel.scopeEntry = entry;
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
  write("apps", panel.manualRefresh?.apps ? ["Rilevamento requisiti in corso…"] : appCheckLines(data.app_checks?.[origin + ":" + entry.id]), "Nessuna app aggiuntiva individuata");
  if (result.status === "ready") requestAppCheck(panel, entry);
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
  const result = await api("scopes", { source_device_id: origin, chat_id: entry.id, scopes });
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
    await saveLinks;
    const result = await api(kind === "chat" ? "chat/hide" : "device/delete", { id });
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
  delete data.appearance?.[(kind === "chat" ? "chat:" : "device:") + id];
  renderMap(); renderLibrary();
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
    panel.hidePopover(); panel.showPopover();
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
function editChat(entry, anchor, origin = sourceId || "master") {
  const existing = [...document.querySelectorAll(".chat-editor")].find(panel => panel.dataset.objectId === entry.id && panel.dataset.sourceDevice === origin);
  if (existing) {
    existing.opener = anchor; bindPanelReference(existing);
    existing.querySelector('[name="label"]').focus();
    return existing;
  }
  const value = data.appearance?.["chat:" + entry.id] || {};
  const scopes = data.chat_settings?.[entry.id]?.scopes || chatLinks.find(v => v.entry.id === entry.id)?.scopes || {};
  const panel = document.createElement("div");
  panel.id = "chat-editor-" + crypto.randomUUID(); panel.classList.add("chat-editor"); panel.opener = anchor; panel.dataset.objectId = entry.id; panel.dataset.sourceDevice = origin;
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
  panel.querySelector('.delete-presentation').onclick = guard(() => removePresentation("chat", entry.id));
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
  const matches = active || !current.device_id || current.device_id === connection.target;
  const modules = matches ? current.module_progress || {} : {};
  const changed = Object.values(modules).some(module => module.changed === true);
  const checking = connection.flow?.phase === 'checking' || data.app_checks?.[key]?.status === 'pending' || !(data.checks_paused ?? data.paused) && data.analyses?.[key]?.status === 'pending';
  const unfinished = Object.values(modules).some(module => module.changed === true && module.progress < 1);
  const remote = connection.source === 'master' ? connection.target : connection.source;
  const paused = changed && unfinished && (data.paused || current.status === 'paused' || current.status === 'failed' || data.connections[remote] === 'offline');
  const phase = paused ? 'paused' : connection.flow?.phase === 'transferring' || matches && current.status === 'running' ? 'transferring' : checking ? 'checking' : 'idle';
  marker.badgeMotion.update({ phase, modules });
}
function renderLink(force = false) {
  if (drawingMap) return;
  if (document.querySelector('.link-badge[data-dragging="true"]')) return;
  if (!force && document.querySelector(".link-badge:hover, .link-badge:focus-within")) return;
  const layers = new Map([...document.querySelectorAll(".chat-link")].map(node => [node.dataset.renderKey, node]));
  const geometry = Object.fromEntries([...document.querySelectorAll(".map-node")].map(node => {
    const rect = node.getBoundingClientRect();
    return [node.dataset.id, [rect.x, rect.y, rect.width, rect.height]];
  }));
  const links = [...chatLinks];
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
  const startSide = fromPort.classList.contains("sync") ? -1 : 1;
  const endSide = toPort.classList.contains("sync") ? -1 : 1;
  const x1 = start.left + start.width / 2 + startSide * (start.width / 2 + 2) - base.left,
    x2 = end.left + end.width / 2 + endSide * (end.width / 2 + 2) - base.left,
    y1 = start.top + start.height / 2 - base.top,
    y2 = end.top + end.height / 2 - base.top;
  const dx = x2 - x1, dy = y2 - y1, distance = Math.max(1, Math.hypot(dx, dy));
  const reach = Math.max(45, Math.min(150, distance * 0.35)) + lane * 80;
  const curve = `M${x1} ${y1} C${x1 + startSide * reach} ${y1} ${x2 + endSide * reach} ${y2} ${x2} ${y2}`;
  const layer = existing || document.createElement("div");
  layer.className = "chat-link";
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
  main.setAttribute("aria-label", "Sposta " + name);
  main.title = "Trascina " + name;
  const open = marker.querySelector(".link-open") || document.createElement("button");
  open.type = "button"; open.className = "link-open icon-button";
  open.setAttribute("aria-label", "Apri " + name); open.title = "Apri console";
  open.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true">' + icons.terminal + '</svg>';
  open.onclick = () => chooseChatTerminal(connection, open);
  const edit = marker.querySelector(".link-edit") || document.createElement("button");
  edit.type = "button";
  edit.className = "link-edit icon-button";
  edit.setAttribute("aria-label", "Modifica " + name);
  edit.title = "Modifica chat";
  edit.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m15 4 5 5-11 11-6 1 1-6zm-2 2 5 5"/></svg>';
  edit.onclick = guard(async () => {
    const cached = connection.source === sourceId ? visibleChats : connection.source === "master" ? data.chats : [];
    const panel = editChat(cached.find(c => c.id === connection.entry.id) || connection.entry, edit, connection.source);
    try {
      const result = await api("catalog", { device_id: connection.source });
      const fresh = result.chats.find(c => c.id === connection.entry.id);
      if (fresh && panel.isConnected) renderScopeDetails(panel, fresh);
    } catch (_) { /* Cached details remain usable while the source is unreachable. */ }
  });
  const play = marker.querySelector(".link-play") || document.createElement("button");
  play.type = "button"; play.className = "link-play icon-button";
  const phase = connection.flow?.phase;
  play.hidden = !["ready", "failed"].includes(phase) || connection.operation !== "sync" && connection.flow?.initialized;
  play.disabled = data.paused || phase === "transferring" || connection.flow?.requested === true;
  play.dataset.ready = String(phase === "ready");
  play.title = phase === "failed" ? "Riprova trasferimento" : "Sincronizza verso la copia rimasta indietro";
  play.setAttribute("aria-label", play.title);
  play.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m8 5 11 7-11 7Z"/></svg>';
  play.onclick = guard(async () => {
    play.disabled = true;
    try { chatLinks = await api("links/play", connection); renderLink(true); }
    finally { if (play.isConnected) renderLink(true); }
  });
  marker.title = name + (connection.flow?.error ? " · " + connection.flow.error : phase === "conflict" ? " · Le copie divergono: scegliere quale mantenere" : phase === "offline" ? " · In attesa del dispositivo" : "");
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
  const bend = curveThroughBadge([{ x: x1, y: y1 }, { x: x1 + startSide * reach, y: y1 }, { x: x2 + endSide * reach, y: y2 }, { x: x2, y: y2 }], midpoint);
  const updateCurve = () => path.setAttribute("d", bend({ x: parseFloat(marker.style.left), y: parseFloat(marker.style.top) }));
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
function acceptsChatDrop(source, target, operation) {
  return source !== target && (source === "master" || target === "master") &&
    (operation === "sync" || operation === "receive");
}
function renderLibrary() {
  const search = $("#search").value.toLocaleLowerCase();
  const focused = document.activeElement?.dataset.chatId;
  const entries = visibleChats.filter(
    (c) => (showHiddenChats || !data.chat_settings?.[c.id]?.hidden) && (
      chatLabel(c).toLocaleLowerCase().includes(search) ||
      c.name.toLocaleLowerCase().includes(search) ||
      c.id.toLocaleLowerCase().includes(search)),
  );
  $("#library-list").replaceChildren();
  if (!entries.length) {
    $("#library-list").innerHTML =
      '<p class="muted">' +
      (search ? "Nessun risultato." : "Nessuna chat disponibile.") +
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
    edit.setAttribute("aria-label", "Personalizza " + chatLabel(entry));
    edit.innerHTML =
      '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m15 4 5 5-11 11-6 1 1-6zm-2 2 5 5"/></svg>';
    edit.onclick = () => editChat(entry, edit);
    row.classList.toggle("hidden-chat", data.chat_settings?.[entry.id]?.hidden === true);
    row.append(button, edit);
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
  const canvasHeight = Math.max(
    height,
    gridHeight + 200,
    ...Object.values(positions).map((p) => p.y + 180),
  );
  return {
    nodeWidth,
    gap,
    columns,
    gridHeight,
    canvasHeight,
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
  node.style.left =
    Math.max(55, Math.min(p.x, $("#map").clientWidth - node.offsetWidth - 55)) +
    "px";
  node.style.top =
    Math.max(
      0,
      Math.min(p.y, $("#nodes").clientHeight - node.offsetHeight),
    ) + "px";
  renderLink();
}
let positionTimer, positionsSaving = false;
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
let mapMotionFrame;
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
    const node = document.createElement("div");
    node.className = "map-node";
    node.dataset.id = device.id;
    node.tabIndex = 0;
    node.setAttribute("role", "button");
    node.innerHTML =
      '<span class="device-symbol">' + iconBody(data.appearance?.["device:" + device.id]?.icon ?? "desktop", device.name) +
      (device.id !== "master" && data.ssh_settings?.[device.id]?.in_lan !== true ? '<span class="remote-badge" role="img" aria-label="Probabilmente remoto" title="Probabilmente remoto"><svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="4" ry="9"/><path d="M3 12h18"/></svg></span>' : '') + '</span>' +
      '<span class="node-name">'  +
      esc(device.name) +
      '</span><i class="status-dot" aria-hidden="true"></i>';
    const color = data.appearance?.["device:" + device.id]?.color ||
      (device.id === "master" ? null : automaticColor(device.id));
    if (color) {
      node.style.setProperty("--device-color", color);
      node.classList.add("colored-device");
    }
    const customize = document.createElement("button");
    customize.className = "node-color icon-button";
    customize.setAttribute("aria-label", "Modifica " + device.name);
    customize.title = "Modifica dispositivo";
    customize.innerHTML =
      '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m15 4 5 5-11 11-6 1 1-6zm-2 2 5 5"/></svg>';
    customize.onclick = (e) => {
      e.stopPropagation();
      editDevice(device, customize);
    };
    node.append(customize);
    $("#nodes").append(node);
    place(node, previous[device.id] || positions[device.id] || defaultPosition(index));
    {
      for (const [operation, label] of [
        ["sync", "Sincronizzazione bidirezionale"],
        ["send", "Invia a questo dispositivo"],
        ["receive", "Ricevi da questo dispositivo"],
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
        if (local) port.title = operation === "send" ? "Invia a un dispositivo" : operation === "receive" ? "Ricevi da un dispositivo" : label;
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
            if (!id) { notice("Seleziona il dispositivo da cui ricevere la chat."); return; }
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
            notice("Trascina su Ricevi o Bidirezionale di un altro dispositivo compatibile.");
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
      if (e.button !== 0 || e.target.closest(".connection-port, .node-color"))
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
      if (!moved) await openDevice(device.id);
    });
    node.onkeydown = guard(async (e) => {
      if (e.target !== node) return;
      if (["Enter", " "].includes(e.key)) {
        e.preventDefault();
        await openDevice(device.id);
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
    const state =
      id === "master" ? "online" : data.connections[id] || "unknown";
    node.className =
      "map-node " +
      state +
      (id === sourceId ? " selected" : "") +
      (id !== "master" || data.appearance?.["device:" + id]?.color ? " colored-device" : "");
    const label = {
      online: "raggiungibile",
      offline: "non raggiungibile",
      checking: "verifica in corso",
      unknown: "non verificato",
    }[state];
    node.title = deviceName(id) + " · " + label;
    node.setAttribute(
      "aria-label",
      deviceName(id) +
        ", " +
        label +
        ". Invio per le chat, frecce per spostare.",
    );
  });
}
$("#reset-map").onclick = () => {
  positions = {};
  savePositions();
  renderMap();
};
$("#check").onclick = guard(() => api("check", {}));
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
async function selectChat(entry, targetId, operation = "copy") {
  const association = { source: sourceId, target: targetId, operation, entry: { id: entry.id, name: entry.name },
    scopes: data.chat_settings?.[entry.id]?.scopes || { chat: true, paths: true, apps: true }, transfer_pending: true };
  try {
    await saveLinks;
    chatLinks = await api("links", association);
    link = null;
    renderLink();
  } catch (error) { notice("Associazione non salvata: " + error.message); }
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
$("#add-device").onclick = () => openDiscovery();
$("#edit-device").onclick = () => editDevice(data.devices.find(d => d.id === sourceId), $("#edit-device"));
const lastConnection = {};
async function poll() {
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
  if (next.shared && !removingPresentation && !positionsSaving && !linkPositionSaving && !drag && !document.querySelector('.map-node[data-dragging="true"], .link-badge[data-dragging="true"]')) {
    const previous = JSON.stringify([data.devices, data.appearance, chatLinks, data.chat_settings, positions, linkPositions]);
    const shared = next.shared;
    if (previous !== JSON.stringify([shared.devices, shared.appearance, shared.links, shared.chat_settings, shared.positions, shared.link_positions])) {
      Object.assign(data, shared); chatLinks = shared.links; positions = shared.positions; linkPositions = shared.link_positions || {};
      renderMap(true); renderLibrary();
    }
  }
  data.connections = next.connections;
  data.job = next.job;
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
setInterval(async () => {
  if (polling) return;
  polling = true;
  try {
    await poll();
  } catch (e) {
    notice(e.message);
  } finally {
    polling = false;
  }
}, 2000);

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
function openDiscovery() {
  closeDeviceSettings();
  let panel = $("#discovery-popover");
  if (!panel) {
    panel = document.createElement("div"); panel.id = "discovery-popover"; panel.opener = $("#add-device");
    panel.setAttribute("popover", "manual"); panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-label", "Collega dispositivo");
    panel.innerHTML = '<button class="icon-button popover-close" type="button" aria-label="Chiudi"><svg viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18"/></svg></button><h2>Collega dispositivo</h2><div class="discovery-content" aria-live="polite"></div><button class="discovery-manual" type="button">Aggiungi manualmente</button>';
    document.body.append(panel); panel.showPopover();
    panel.querySelector('.popover-close').onclick = () => panel.remove();
    panel.querySelector('.discovery-manual').onclick = () => { panel.remove(); editDevice(); };
  }
  renderDiscovery(); positionContext(panel, $("#add-device"));
}
function renderDiscovery() {
  const panel = $("#discovery-popover"); if (!panel) return;
  const content = panel.querySelector('.discovery-content');
  const html = (discovery.setup_needed || discovery.network?.status === 'pending' ? '<button type="button" class="discovery-network" ' + (discovery.network?.status === 'pending' ? 'disabled' : '') + '>Prepara collegamento</button><small>' + (discovery.network?.status === 'pending' ? 'Attendi la conferma di Windows…' : 'La rete virtuale richiede un inoltro. Windows chiederà la conferma amministrativa.') + '</small>' : '') +
    (discovery.network?.error ? '<p class="discovery-error">' + esc(discovery.network.error) + '</p>' : '') +
    (discovery.error ? '<p class="discovery-error">' + esc(discovery.error) + '</p>' : '') +
    (discovery.ignored ? '<button type="button" class="discovery-restore">Mostra dispositivi rifiutati</button>' : '') +
    (discovery.peers.length ? discovery.peers.map(peer => '<section class="discovery-peer"><strong>' + esc(peer.name) + '</strong><small>' + esc(peer.address) + '</small>' +
      (peer.connected ? '<p>Collegamento autorizzato</p>' : '<p>Confronta questo codice sui due dispositivi.</p><strong class="pairing-code">' + esc(peer.code) + '</strong>' +
        (peer.accepted ? '<p>' + (peer.remote_accepted ? 'Autorizzato. Preparazione SSH in corso.' : 'In attesa della conferma sull’altro dispositivo.') + '</p>' : '<button type="button" class="primary discovery-accept" data-peer="' + esc(peer.id) + '">Il codice coincide · Accetta</button>') +
        (peer.ssh_ready === false ? '<small>SSH da attivare sul dispositivo.</small>' : '') +
        '<button type="button" class="discovery-reject" data-peer="' + esc(peer.id) + '">Rifiuta</button>') + '</section>').join('') : '<p>Ricerca di Oaclis sulla porta 23232…</p>');
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
