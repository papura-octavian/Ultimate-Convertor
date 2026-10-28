import { t, lang, setLang, applyStatic } from "./i18n.js";
import { icon, hydrateIcons } from "./icons.js";
import { initTools, refreshTools } from "./tools.js";

// Outside the app (a plain browser) a tiny fake backend keeps the UI usable for design work.
const tauri = window.__TAURI__ ?? (await import("./dev-mock.js")).default;
const { invoke } = tauri.core;
const { listen } = tauri.event;

const $ = (sel) => document.querySelector(sel);
const KINDS = ["image", "video", "audio", "data", "document"];
const FORMAT_CLOUD = {
  image: ["HEIC", "AVIF", "WEBP", "PNG", "JPG", "GIF", "TIFF", "BMP", "ICO", "TGA", "QOI", "EXR", "HDR", "PDF"],
  video: ["MP4", "MKV", "MOV", "WEBM", "AVI", "WMV", "FLV", "MTS", "3GP"],
  audio: ["MP3", "WAV", "FLAC", "M4A", "OGG", "OPUS", "WMA", "AIFF"],
  data: ["JSON", "YAML", "TOML", "CSV"],
  document: ["MD → HTML"],
};

const store = {
  get(key, fallback) {
    try {
      const v = localStorage.getItem(key);
      return v === null ? fallback : JSON.parse(v);
    } catch {
      return fallback;
    }
  },
  set(key, value) {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {}
  },
};

const state = {
  files: new Map(), // id -> file
  order: [],
  nextId: 1,
  running: false,
  canceling: false,
  groupTarget: store.get("groupTarget", {}),
  settings: {
    quality: 85,
    maxSide: 0,
    maxSideMode: "0",
    outMode: "same",
    outDir: null,
    fast: true,
    mergePdf: false,
    ...store.get("settings", {}),
  },
  lastOutput: null,
};

/* ---------- formatting ---------- */

function fmtSize(bytes) {
  if (bytes == null) return "";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = bytes;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

function fmtTime(ms) {
  if (ms < 1000) return `${ms} ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)} s`;
  return `${Math.floor(s / 60)} min ${Math.round(s % 60)} s`;
}

const up = (ext) => ext.toUpperCase();

function toast(msg) {
  const el = $("#toast");
  el.textContent = msg;
  el.classList.add("show");
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => el.classList.remove("show"), 2600);
}

/* ---------- adding files ---------- */

async function addPaths(paths) {
  if (!paths?.length) return;
  const { files, skipped } = await invoke("inspect", { paths });
  const known = new Set([...state.files.values()].map((f) => f.path));
  let added = 0;
  for (const f of files) {
    if (known.has(f.path)) continue;
    known.add(f.path);
    const preferred = state.groupTarget[f.kind];
    const file = {
      ...f,
      id: state.nextId++,
      target: preferred && f.targets.includes(preferred) ? preferred : f.default_target,
      state: "idle",
      progress: 0,
    };
    state.files.set(file.id, file);
    state.order.push(file.id);
    added++;
    queueThumb(file);
  }
  if (skipped) toast(t("skipped", skipped));
  else if (!added && !files.length) toast(t("noneSupported"));
  $("#summary").classList.add("hidden");
  renderAll();
}

async function pickFiles() {
  const picked = await tauri.dialog.open({ multiple: true, directory: false });
  if (picked) addPaths(Array.isArray(picked) ? picked : [picked]);
}

async function pickFolder() {
  const picked = await tauri.dialog.open({ multiple: true, directory: true });
  if (picked) addPaths(Array.isArray(picked) ? picked : [picked]);
}

/* ---------- thumbnails (a few at a time) ---------- */

const thumbQueue = [];
let thumbsActive = 0;

function queueThumb(file) {
  if (file.kind !== "image" && file.kind !== "video") return;
  thumbQueue.push(file);
  pumpThumbs();
}

function pumpThumbs() {
  while (thumbsActive < 3 && thumbQueue.length) {
    const file = thumbQueue.shift();
    if (!state.files.has(file.id)) continue;
    thumbsActive++;
    invoke("thumbnail", { path: file.path })
      .then((url) => {
        if (!url || !state.files.has(file.id)) return;
        file.thumb = url;
        const box = document.querySelector(`.file[data-id="${file.id}"] .thumb`);
        if (box) box.innerHTML = `<img src="${url}" alt="" />`;
      })
      .catch(() => {})
      .finally(() => {
        thumbsActive--;
        pumpThumbs();
      });
  }
}

/* ---------- rendering ---------- */

const rows = new Map(); // id -> <li>

function pending() {
  return state.order.map((id) => state.files.get(id)).filter((f) => f.state !== "done" && f.state !== "working");
}

function renderAll() {
  const has = state.order.length > 0;
  $("#dropzone").classList.toggle("hidden", has);
  $("#worklist").classList.toggle("hidden", !has);
  $("#list-count").textContent = t("filesCount", state.order.length);
  renderGroups();
  renderList();
  renderConvertButton();
}

function renderGroups() {
  const box = $("#groups");
  box.innerHTML = "";
  for (const kind of KINDS) {
    const files = state.order.map((id) => state.files.get(id)).filter((f) => f.kind === kind);
    if (!files.length) continue;
    const targets = [...new Set(files.flatMap((f) => f.targets))];
    const used = new Set(files.map((f) => f.target));
    const row = document.createElement("div");
    row.className = `group k-${kind}`;
    row.innerHTML = `<div class="group-name"><span class="kind-badge"><i>${icon(kind)}</i></span>${t("kind_" + kind)} <em>${files.length}</em></div>`;
    const chips = document.createElement("div");
    chips.className = "chips";
    for (const target of targets) {
      const b = document.createElement("button");
      b.className = "chip";
      if (used.size === 1 && used.has(target)) b.classList.add("on");
      else if (used.has(target)) b.classList.add("mixed");
      b.textContent = up(target);
      b.disabled = state.running;
      b.onclick = () => setGroupTarget(kind, target);
      chips.append(b);
    }
    row.append(chips);
    box.append(row);
  }
}

function setGroupTarget(kind, target) {
  state.groupTarget[kind] = target;
  store.set("groupTarget", state.groupTarget);
  for (const f of state.files.values()) {
    if (f.kind === kind && f.targets.includes(target) && f.target !== target) {
      f.target = target;
      if (f.state !== "working") f.state = "idle";
    }
  }
  renderAll();
}

function renderList() {
  const list = $("#files");
  const alive = new Set(state.order);
  for (const [id, li] of rows) {
    if (!alive.has(id)) {
      li.remove();
      rows.delete(id);
    }
  }
  for (const id of state.order) {
    let li = rows.get(id);
    if (!li) {
      li = createRow(state.files.get(id));
      rows.set(id, li);
      list.append(li);
    }
    updateRow(state.files.get(id));
  }
}

function createRow(file) {
  const li = document.createElement("li");
  li.className = `file k-${file.kind}`;
  li.dataset.id = file.id;
  li.innerHTML = `
    <div class="thumb">${file.thumb ? `<img src="${file.thumb}" alt="" />` : `<span class="kind-badge"><i>${icon(file.kind)}</i></span>`}</div>
    <div class="file-info"><div class="file-name"></div><div class="file-meta"></div></div>
    <select class="target"></select>
    <div class="status"></div>
    <button class="icon-btn remove"><i>${icon("x")}</i></button>
    <span class="bar"></span>`;
  li.querySelector(".file-name").textContent = file.name;
  li.querySelector(".file-name").title = file.path;
  const select = li.querySelector(".target");
  for (const target of file.targets) {
    const o = document.createElement("option");
    o.value = target;
    o.textContent = up(target);
    select.append(o);
  }
  select.onchange = () => {
    file.target = select.value;
    file.state = "idle";
    renderGroups();
    updateRow(file);
    renderConvertButton();
  };
  li.querySelector(".remove").onclick = () => {
    state.files.delete(file.id);
    state.order = state.order.filter((id) => id !== file.id);
    renderAll();
  };
  return li;
}

function updateRow(file) {
  const li = rows.get(file.id);
  if (!li) return;
  li.className = `file k-${file.kind} ${file.state}`;
  const select = li.querySelector(".target");
  select.value = file.target;
  select.disabled = state.running;
  li.querySelector(".remove").disabled = file.state === "working";
  li.querySelector(".remove").title = t("remove");

  const meta = li.querySelector(".file-meta");
  if (file.state === "done" && file.outSize != null) {
    const delta = file.size ? Math.round(((file.outSize - file.size) / file.size) * 100) : 0;
    const sign = delta > 0 ? "+" : "";
    // Size change only means something for media; text formats just change shape.
    const change = file.kind === "data" || file.kind === "document" ? "" : ` <span>(${sign}${delta}%)</span>`;
    meta.innerHTML = `${up(file.ext)} ${fmtSize(file.size)} <span class="arrow">→</span> ${up(file.target)} ${fmtSize(file.outSize)}${change}`;
  } else if (file.state === "error") {
    meta.innerHTML = "";
    const err = document.createElement("span");
    err.className = "err";
    err.textContent = file.error;
    meta.append(err);
    meta.title = file.error;
  } else {
    meta.textContent = `${up(file.ext)} · ${fmtSize(file.size)}`;
  }

  const status = li.querySelector(".status");
  const bar = li.querySelector(".bar");
  bar.classList.toggle("indeterminate", file.state === "working" && file.progress < 0);
  bar.style.width = file.state === "working" && file.progress >= 0 ? `${Math.round(file.progress * 100)}%` : file.state === "done" ? "100%" : "0";
  switch (file.state) {
    case "queued":
      status.textContent = t("queued");
      break;
    case "working":
      status.textContent = file.progress >= 0 ? `${Math.round(file.progress * 100)}%` : t("working");
      break;
    case "done": {
      status.innerHTML = "";
      const link = document.createElement("button");
      link.className = "link";
      link.innerHTML = `<i>${icon("check")}</i>${t("show")}`;
      link.onclick = () => tauri.opener.revealItemInDir(file.output);
      status.append(link);
      break;
    }
    case "error":
      status.innerHTML = `<i>${icon("alert")}</i>${t("error")}`;
      break;
    case "canceled":
      status.innerHTML = `<i>${icon("ban")}</i>${t("canceled")}`;
      break;
    default:
      status.textContent = "";
  }
}

function renderConvertButton() {
  const btn = $("#convert");
  const label = $("#convert-label");
  const iconEl = btn.querySelector("i");
  if (state.running) {
    btn.disabled = state.canceling;
    btn.classList.add("stop");
    label.textContent = state.canceling ? t("canceling") : t("cancel");
    iconEl.innerHTML = icon("stop");
  } else {
    const n = pending().length;
    btn.disabled = n === 0;
    btn.classList.remove("stop");
    label.textContent = n ? t("convertN", n) : t("convert");
    iconEl.innerHTML = icon("zap");
  }
  $("#btn-clear-done").classList.toggle("hidden", ![...state.files.values()].some((f) => f.state === "done"));
}

/* ---------- converting ---------- */

async function startOrStop() {
  if (state.running) {
    state.canceling = true;
    renderConvertButton();
    await invoke("cancel");
    return;
  }
  const todo = pending();
  if (!todo.length) return;
  const s = state.settings;
  if (s.outMode === "other" && !s.outDir) {
    await chooseOutDir();
    if (!state.settings.outDir) return;
  }
  for (const f of todo) {
    f.state = "queued";
    f.progress = 0;
    f.error = null;
  }
  state.running = true;
  state.canceling = false;
  $("#summary").classList.add("hidden");
  renderAll();
  try {
    await invoke("convert", {
      jobs: todo.map((f) => ({ id: f.id, path: f.path, target: f.target })),
      options: {
        quality: s.quality,
        maxSide: s.maxSide,
        outDir: s.outMode === "other" ? s.outDir : null,
        fast: s.fast,
        mergePdf: s.mergePdf,
      },
    });
  } catch (e) {
    state.running = false;
    for (const f of todo) f.state = "idle";
    toast(String(e));
    renderAll();
  }
}

listen("job", ({ payload: e }) => {
  const file = state.files.get(e.id);
  if (!file) return;
  file.state = e.state;
  file.progress = e.progress;
  if (e.output) {
    file.output = e.output;
    state.lastOutput = e.output;
  }
  if (e.outSize != null) file.outSize = e.outSize;
  file.error = e.error;
  updateRow(file);
});

listen("batch", ({ payload: e }) => {
  state.running = false;
  state.canceling = false;
  renderAll();
  const box = $("#summary");
  const parts = [`<span>${t("summary", e.done, `<b>${fmtTime(e.millis)}</b>`)}</span>`];
  if (e.failed) parts.push(`<span class="bad">${t("summaryFailed", e.failed)}</span>`);
  if (e.canceled) parts.push(`<span>${t("summaryCanceled", e.canceled)}</span>`);
  box.innerHTML = parts.join("");
  if (e.done && state.lastOutput) {
    const b = document.createElement("button");
    b.innerHTML = `<i data-icon="folder"></i>${t("openFolder")}`;
    const out = state.lastOutput;
    b.onclick = () => tauri.opener.revealItemInDir(out);
    box.append(b);
    hydrateIcons(box);
  }
  box.classList.remove("hidden");
});

/* ---------- settings ---------- */

function saveSettings() {
  store.set("settings", state.settings);
}

function bindSettings() {
  const s = state.settings;
  const q = $("#quality");
  const paintQuality = () => {
    $("#quality-out").textContent = q.value;
    q.style.setProperty("--fill", `${((q.value - q.min) / (q.max - q.min)) * 100}%`);
  };
  q.value = s.quality;
  paintQuality();
  q.oninput = () => {
    s.quality = Number(q.value);
    paintQuality();
    saveSettings();
  };

  const seg = $("#maxside");
  const custom = $("#maxside-custom");
  const paintMax = () => {
    seg.querySelectorAll("button").forEach((b) => b.classList.toggle("on", b.dataset.v === s.maxSideMode));
    custom.classList.toggle("hidden", s.maxSideMode !== "custom");
    if (s.maxSideMode === "custom") custom.value = s.maxSide || "";
  };
  seg.onclick = (e) => {
    const b = e.target.closest("button");
    if (!b) return;
    s.maxSideMode = b.dataset.v;
    s.maxSide = b.dataset.v === "custom" ? Number(custom.value) || 0 : Number(b.dataset.v);
    paintMax();
    if (b.dataset.v === "custom") custom.focus();
    saveSettings();
  };
  custom.oninput = () => {
    s.maxSide = Math.max(0, Math.round(Number(custom.value) || 0));
    saveSettings();
  };
  paintMax();

  const outSeg = $("#outmode");
  outSeg.onclick = async (e) => {
    const b = e.target.closest("button");
    if (!b) return;
    if (b.dataset.v === "other" && !s.outDir) await chooseOutDir();
    else s.outMode = b.dataset.v;
    paintOut();
    saveSettings();
  };
  $("#outdir").onclick = chooseOutDir;
  paintOut();

  for (const [id, key] of [
    ["#fast", "fast"],
    ["#merge-pdf", "mergePdf"],
  ]) {
    const el = $(id);
    el.checked = s[key];
    el.onchange = () => {
      s[key] = el.checked;
      saveSettings();
    };
  }
}

function paintOut() {
  const s = state.settings;
  $("#outmode")
    .querySelectorAll("button")
    .forEach((b) => b.classList.toggle("on", b.dataset.v === s.outMode));
  const dir = $("#outdir");
  dir.classList.toggle("hidden", s.outMode !== "other" || !s.outDir);
  $("#outdir-name").textContent = s.outDir ?? "";
  dir.title = s.outDir ?? "";
}

async function chooseOutDir() {
  const picked = await tauri.dialog.open({ directory: true, multiple: false });
  if (picked) {
    state.settings.outDir = picked;
    state.settings.outMode = "other";
    saveSettings();
  }
  paintOut();
}

/* ---------- chrome: tabs, language, drag & drop ---------- */

function moveGlider() {
  const active = document.querySelector(".tab.active");
  const glider = document.querySelector(".tab-glider");
  glider.style.width = `${active.offsetWidth}px`;
  glider.style.transform = `translateX(${active.offsetLeft}px)`;
}

function showTab(name) {
  document.querySelectorAll(".tab").forEach((b) => b.classList.toggle("active", b.dataset.tab === name));
  document.querySelectorAll(".panel").forEach((p) => p.classList.toggle("active", p.id === `panel-${name}`));
  moveGlider();
}

function applyLanguage() {
  applyStatic();
  document.querySelectorAll("[data-lang]").forEach((b) => b.classList.toggle("on", b.dataset.lang === lang));
  renderAll();
  for (const f of state.files.values()) updateRow(f);
  refreshTools();
  requestAnimationFrame(moveGlider);
}

function renderFormatCloud() {
  const box = $("#format-cloud");
  box.innerHTML = KINDS.flatMap((k) => FORMAT_CLOUD[k].map((f) => `<span class="k-${k}">${f}</span>`)).join("");
}

async function bindDragDrop() {
  const main = document.querySelector(".files-main");
  await tauri.webview.getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (payload.type === "enter" || payload.type === "over") {
      if (!state.running) {
        showTab("files");
        main.classList.add("dragging");
      }
    } else if (payload.type === "drop") {
      main.classList.remove("dragging");
      if (!state.running) addPaths(payload.paths);
    } else {
      main.classList.remove("dragging");
    }
  });
}

function bindActions() {
  document.addEventListener("click", (e) => {
    const action = e.target.closest("[data-action]")?.dataset.action;
    if (action === "pick-files") pickFiles();
    else if (action === "pick-folder") pickFolder();
    else if (action === "clear" && !state.running) {
      state.files.clear();
      state.order = [];
      $("#summary").classList.add("hidden");
      renderAll();
    } else if (action === "clear-done") {
      for (const f of [...state.files.values()]) if (f.state === "done") state.files.delete(f.id);
      state.order = state.order.filter((id) => state.files.has(id));
      renderAll();
    }
  });
  $("#convert").onclick = startOrStop;
  document.querySelectorAll(".tab").forEach((b) => (b.onclick = () => showTab(b.dataset.tab)));
  document.querySelectorAll("[data-lang]").forEach(
    (b) =>
      (b.onclick = () => {
        setLang(b.dataset.lang);
        applyLanguage();
      }),
  );
  document.addEventListener("keydown", (e) => {
    if (e.ctrlKey && e.key.toLowerCase() === "o") {
      e.preventDefault();
      pickFiles();
    } else if (e.ctrlKey && e.key === "Enter") {
      e.preventDefault();
      startOrStop();
    }
  });
  window.addEventListener("resize", moveGlider);
}

hydrateIcons();
renderFormatCloud();
bindSettings();
bindActions();
initTools();
applyLanguage();
bindDragDrop();
