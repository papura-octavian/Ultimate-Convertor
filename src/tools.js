import { t, lang } from "./i18n.js";

const $ = (sel) => document.querySelector(sel);

/* ---------- units ---------- */

// Factor to the category's base unit; temperature and fuel use functions.
const UNITS = {
  length: [
    ["mm", "mm", 0.001],
    ["cm", "cm", 0.01],
    ["m", "m", 1],
    ["km", "km", 1000],
    ["in", "in", 0.0254],
    ["ft", "ft", 0.3048],
    ["yd", "yd", 0.9144],
    ["mi", "mi", 1609.344],
    ["nmi", "nmi", 1852],
    ["µm", "µm", 1e-6],
  ],
  mass: [
    ["mg", "mg", 1e-6],
    ["g", "g", 0.001],
    ["kg", "kg", 1],
    ["t", "t", 1000],
    ["oz", "oz", 0.028349523125],
    ["lb", "lb", 0.45359237],
    ["st", "st", 6.35029318],
    ["ct", "ct", 0.0002],
  ],
  temperature: [
    ["C", "°C", null],
    ["F", "°F", null],
    ["K", "K", null],
  ],
  volume: [
    ["ml", "ml", 0.001],
    ["l", "l", 1],
    ["m3", "m³", 1000],
    ["tsp", "tsp", 0.00492892159375],
    ["tbsp", "tbsp", 0.01478676478125],
    ["cup", "cup (US)", 0.2365882365],
    ["floz", "fl oz (US)", 0.0295735295625],
    ["gal", "gal (US)", 3.785411784],
    ["galuk", "gal (UK)", 4.54609],
  ],
  area: [
    ["cm2", "cm²", 1e-4],
    ["m2", "m²", 1],
    ["ha", "ha", 10000],
    ["km2", "km²", 1e6],
    ["ft2", "ft²", 0.09290304],
    ["ac", "acre", 4046.8564224],
    ["ar", "ar", 100],
  ],
  speed: [
    ["kmh", "km/h", 1 / 3.6],
    ["ms", "m/s", 1],
    ["mph", "mph", 0.44704],
    ["kn", "kn", 0.514444444],
    ["fts", "ft/s", 0.3048],
  ],
  time: [
    ["ms", "ms", 0.001],
    ["s", "s", 1],
    ["min", "min", 60],
    ["h", "h", 3600],
    ["d", "d", 86400],
    ["wk", "wk", 604800],
    ["yr", "yr", 31557600],
  ],
  data: [
    ["b", "bit", 0.125],
    ["B", "B", 1],
    ["KB", "KB", 1000],
    ["MB", "MB", 1e6],
    ["GB", "GB", 1e9],
    ["TB", "TB", 1e12],
    ["KiB", "KiB", 1024],
    ["MiB", "MiB", 1024 ** 2],
    ["GiB", "GiB", 1024 ** 3],
    ["TiB", "TiB", 1024 ** 4],
  ],
  pressure: [
    ["Pa", "Pa", 1],
    ["kPa", "kPa", 1000],
    ["bar", "bar", 100000],
    ["atm", "atm", 101325],
    ["psi", "psi", 6894.757293168],
    ["mmHg", "mmHg", 133.322387415],
  ],
  energy: [
    ["J", "J", 1],
    ["kJ", "kJ", 1000],
    ["cal", "cal", 4.184],
    ["kcal", "kcal", 4184],
    ["Wh", "Wh", 3600],
    ["kWh", "kWh", 3.6e6],
    ["BTU", "BTU", 1055.05585262],
  ],
  power: [
    ["W", "W", 1],
    ["kW", "kW", 1000],
    ["hp", "hp (CP)", 735.49875],
    ["hpi", "hp (mech.)", 745.69987158227],
  ],
  angle: [
    ["deg", "°", Math.PI / 180],
    ["rad", "rad", 1],
    ["grad", "grad", Math.PI / 200],
    ["turn", "turn", 2 * Math.PI],
  ],
  fuel: [
    ["l100", "l/100 km", null],
    ["kml", "km/l", null],
    ["mpg", "mpg (US)", null],
    ["mpguk", "mpg (UK)", null],
  ],
};

const DEFAULT_PAIR = {
  length: ["m", "ft"],
  mass: ["kg", "lb"],
  temperature: ["C", "F"],
  volume: ["l", "gal"],
  area: ["m2", "ft2"],
  speed: ["kmh", "mph"],
  time: ["h", "min"],
  data: ["GB", "GiB"],
  pressure: ["bar", "psi"],
  energy: ["kcal", "kJ"],
  power: ["kW", "hp"],
  angle: ["deg", "rad"],
  fuel: ["l100", "mpg"],
};

function toBase(cat, unit, v) {
  if (cat === "temperature") return unit === "C" ? v : unit === "F" ? ((v - 32) * 5) / 9 : v - 273.15;
  if (cat === "fuel") {
    // base: km per litre
    if (unit === "kml") return v;
    if (unit === "l100") return 100 / v;
    if (unit === "mpg") return v * 0.425143707;
    return v * 0.354006189;
  }
  return v * UNITS[cat].find((u) => u[0] === unit)[2];
}

function fromBase(cat, unit, v) {
  if (cat === "temperature") return unit === "C" ? v : unit === "F" ? (v * 9) / 5 + 32 : v + 273.15;
  if (cat === "fuel") {
    if (unit === "kml") return v;
    if (unit === "l100") return 100 / v;
    if (unit === "mpg") return v / 0.425143707;
    return v / 0.354006189;
  }
  return v / UNITS[cat].find((u) => u[0] === unit)[2];
}

function parseNum(s) {
  const v = Number(String(s).trim().replace(/\s/g, "").replace(",", "."));
  return String(s).trim() === "" ? NaN : v;
}

function fmtNum(v) {
  if (!Number.isFinite(v)) return "—";
  if (v === 0) return "0";
  const abs = Math.abs(v);
  if (abs >= 1e15 || abs < 1e-6) return v.toExponential(6).replace(/\.?0+e/, "e");
  const s = Number(v.toPrecision(12)).toLocaleString(lang === "ro" ? "ro-RO" : "en-US", { maximumFractionDigits: 10 });
  return s;
}

function plainNum(v) {
  if (!Number.isFinite(v)) return "";
  return String(Number(v.toPrecision(12)));
}

const units = { cat: "length", a: "m", b: "ft" };

function renderUnits() {
  const cats = $("#unit-cats");
  cats.innerHTML = "";
  for (const cat of Object.keys(UNITS)) {
    const b = document.createElement("button");
    b.className = "chip" + (cat === units.cat ? " on" : "");
    b.textContent = t("u_" + cat);
    b.onclick = () => {
      units.cat = cat;
      [units.a, units.b] = DEFAULT_PAIR[cat];
      renderUnits();
      calcUnits("a");
    };
    cats.append(b);
  }
  for (const side of ["a", "b"]) {
    const sel = $(`#unit-${side}-sel`);
    sel.innerHTML = UNITS[units.cat].map(([id, label]) => `<option value="${id}">${label}</option>`).join("");
    sel.value = units[side];
  }
}

function calcUnits(from) {
  const other = from === "a" ? "b" : "a";
  const v = parseNum($(`#unit-${from}`).value);
  const base = toBase(units.cat, units[from], v);
  $(`#unit-${other}`).value = Number.isFinite(v) ? plainNum(fromBase(units.cat, units[other], base)) : "";
  const table = $("#unit-table");
  table.innerHTML = "";
  if (!Number.isFinite(v)) return;
  for (const [id, label] of UNITS[units.cat]) {
    const row = document.createElement("div");
    row.className = "unit-row" + (id === units.b ? " on" : "");
    row.innerHTML = `<span></span><b></b>`;
    row.firstChild.textContent = label;
    row.lastChild.textContent = fmtNum(fromBase(units.cat, id, base));
    row.onclick = () => {
      units.b = id;
      $("#unit-b-sel").value = id;
      calcUnits("a");
    };
    table.append(row);
  }
}

function initUnits() {
  $("#unit-a").oninput = () => calcUnits("a");
  $("#unit-b").oninput = () => {
    // Typing on the right converts back into the left, the table follows the left value.
    const v = parseNum($("#unit-b").value);
    $("#unit-a").value = Number.isFinite(v) ? plainNum(fromBase(units.cat, units.a, toBase(units.cat, units.b, v))) : "";
    calcUnits("a");
  };
  $("#unit-a-sel").onchange = (e) => {
    units.a = e.target.value;
    calcUnits("a");
  };
  $("#unit-b-sel").onchange = (e) => {
    units.b = e.target.value;
    calcUnits("a");
  };
  $("#unit-swap").onclick = () => {
    [units.a, units.b] = [units.b, units.a];
    $("#unit-a").value = $("#unit-b").value;
    renderUnits();
    calcUnits("a");
  };
}

/* ---------- number bases ---------- */

const BASES = [
  ["decimal", 10, /^-?\d+$/],
  ["hex", 16, /^-?(0x)?[0-9a-f]+$/i],
  ["binary", 2, /^-?(0b)?[01]+$/i],
  ["octal", 8, /^-?(0o)?[0-7]+$/i],
];

function toRoman(n) {
  if (n <= 0n || n >= 4000n) return "";
  let num = Number(n);
  const map = [
    [1000, "M"], [900, "CM"], [500, "D"], [400, "CD"], [100, "C"], [90, "XC"],
    [50, "L"], [40, "XL"], [10, "X"], [9, "IX"], [5, "V"], [4, "IV"], [1, "I"],
  ];
  let out = "";
  for (const [v, s] of map) while (num >= v) (out += s), (num -= v);
  return out;
}

function fromRoman(s) {
  const r = s.trim().toUpperCase();
  if (!/^M{0,3}(CM|CD|D?C{0,3})(XC|XL|L?X{0,3})(IX|IV|V?I{0,3})$/.test(r) || !r) return null;
  const val = { I: 1, V: 5, X: 10, L: 50, C: 100, D: 500, M: 1000 };
  let total = 0;
  for (let i = 0; i < r.length; i++) {
    const v = val[r[i]];
    total += v < (val[r[i + 1]] || 0) ? -v : v;
  }
  return BigInt(total);
}

function parseBig(str, base) {
  let s = str.trim().replace(/[\s_]/g, "").toLowerCase();
  const neg = s.startsWith("-");
  if (neg) s = s.slice(1);
  s = s.replace(/^0[xbo]/, "");
  let n = 0n;
  const B = BigInt(base);
  for (const ch of s) n = n * B + BigInt(parseInt(ch, base));
  return neg ? -n : n;
}

function renderNumbers() {
  const grid = $("#num-grid");
  grid.innerHTML = "";
  for (const [key] of [...BASES, ["roman"]]) {
    const f = document.createElement("label");
    f.className = "num-field";
    f.innerHTML = `<span><em></em><button class="copy"></button></span><input spellcheck="false" />`;
    f.querySelector("em").textContent = t(key);
    f.querySelector("em").style.fontStyle = "normal";
    f.querySelector(".copy").textContent = t("copy");
    f.dataset.key = key;
    grid.append(f);
  }
  grid.oninput = (e) => {
    const field = e.target.closest(".num-field");
    if (field) setNumber(field.dataset.key, e.target.value);
  };
  grid.onclick = (e) => {
    const btn = e.target.closest(".copy");
    if (!btn) return;
    e.preventDefault();
    copyText(btn.closest(".num-field").querySelector("input").value, btn);
  };
  setNumber("decimal", "255");
}

function setNumber(key, raw) {
  const fields = [...document.querySelectorAll("#num-grid .num-field")];
  const self = fields.find((f) => f.dataset.key === key);
  let n = null;
  if (key === "roman") n = fromRoman(raw);
  else {
    const [, base, re] = BASES.find((b) => b[0] === key);
    if (re.test(raw.trim().replace(/[\s_]/g, ""))) n = parseBig(raw, base);
  }
  self.classList.toggle("bad", n === null && raw.trim() !== "");
  if (n === null) return;
  for (const f of fields) {
    if (f === self) continue;
    const input = f.querySelector("input");
    const k = f.dataset.key;
    f.classList.remove("bad");
    if (k === "roman") input.value = toRoman(n);
    else {
      const base = BASES.find((b) => b[0] === k)[1];
      const neg = n < 0n;
      let s = (neg ? -n : n).toString(base);
      if (k === "hex") s = s.toUpperCase();
      if (k === "binary") s = s.replace(/\B(?=(\d{4})+(?!\d))/g, " ");
      input.value = (neg ? "-" : "") + s;
    }
  }
  if (self.querySelector("input").value !== raw) self.querySelector("input").value = raw;
}

function initBase64() {
  const text = $("#b64-text");
  const code = $("#b64-code");
  text.oninput = () => {
    const bytes = new TextEncoder().encode(text.value);
    let bin = "";
    for (const b of bytes) bin += String.fromCharCode(b);
    code.value = btoa(bin);
    code.classList.remove("bad");
  };
  code.oninput = () => {
    try {
      const clean = code.value.replace(/\s/g, "").replace(/-/g, "+").replace(/_/g, "/");
      const bin = atob(clean);
      text.value = new TextDecoder("utf-8", { fatal: true }).decode(Uint8Array.from(bin, (c) => c.charCodeAt(0)));
      code.style.borderColor = "";
    } catch {
      code.style.borderColor = "var(--red)";
    }
  };
}

/* ---------- colors ---------- */

const color = { r: 139, g: 123, b: 255 };

const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
const hex2 = (n) => Math.round(n).toString(16).padStart(2, "0");
const toHex = ({ r, g, b }) => `#${hex2(r)}${hex2(g)}${hex2(b)}`.toUpperCase();

function rgbToHsl({ r, g, b }) {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  let h = 0;
  let s = 0;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
    h *= 60;
  }
  return { h: Math.round(h), s: Math.round(s * 100), l: Math.round(l * 100) };
}

function hslToRgb(h, s, l) {
  s /= 100;
  l /= 100;
  const k = (n) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return { r: Math.round(f(0) * 255), g: Math.round(f(8) * 255), b: Math.round(f(4) * 255) };
}

function rgbToHsv({ r, g, b }) {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  const h = rgbToHsl({ r: r * 255, g: g * 255, b: b * 255 }).h;
  return { h, s: Math.round(max ? (d / max) * 100 : 0), v: Math.round(max * 100) };
}

function hsvToRgb(h, s, v) {
  s /= 100;
  v /= 100;
  const f = (n) => {
    const k = (n + h / 60) % 6;
    return v - v * s * Math.max(0, Math.min(k, 4 - k, 1));
  };
  return { r: Math.round(f(5) * 255), g: Math.round(f(3) * 255), b: Math.round(f(1) * 255) };
}

function rgbToCmyk({ r, g, b }) {
  const k = 1 - Math.max(r, g, b) / 255;
  if (k >= 1) return { c: 0, m: 0, y: 0, k: 100 };
  const f = (x) => Math.round(((1 - x / 255 - k) / (1 - k)) * 100);
  return { c: f(r), m: f(g), y: f(b), k: Math.round(k * 100) };
}

function luminance({ r, g, b }) {
  const ch = (c) => {
    c /= 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b);
}

const contrast = (l1, l2) => (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);

const nums = (s) => (s.match(/-?\d+(\.\d+)?/g) || []).map(Number);

const COLOR_FIELDS = [
  ["HEX", () => toHex(color), (s) => {
    let h = s.trim().replace(/^#/, "");
    if (/^[0-9a-f]{3}$/i.test(h)) h = h.replace(/./g, "$&$&");
    if (!/^[0-9a-f]{6}$/i.test(h)) return null;
    return { r: parseInt(h.slice(0, 2), 16), g: parseInt(h.slice(2, 4), 16), b: parseInt(h.slice(4, 6), 16) };
  }],
  ["RGB", () => `rgb(${color.r}, ${color.g}, ${color.b})`, (s) => {
    const n = nums(s);
    return n.length >= 3 && n.slice(0, 3).every((v) => v >= 0 && v <= 255) ? { r: n[0], g: n[1], b: n[2] } : null;
  }],
  ["HSL", () => {
    const { h, s, l } = rgbToHsl(color);
    return `hsl(${h}, ${s}%, ${l}%)`;
  }, (s) => {
    const n = nums(s);
    return n.length >= 3 ? hslToRgb(((n[0] % 360) + 360) % 360, clamp(n[1], 0, 100), clamp(n[2], 0, 100)) : null;
  }],
  ["HSV", () => {
    const { h, s, v } = rgbToHsv(color);
    return `hsv(${h}, ${s}%, ${v}%)`;
  }, (s) => {
    const n = nums(s);
    return n.length >= 3 ? hsvToRgb(((n[0] % 360) + 360) % 360, clamp(n[1], 0, 100), clamp(n[2], 0, 100)) : null;
  }],
  ["CMYK", () => {
    const { c, m, y, k } = rgbToCmyk(color);
    return `cmyk(${c}%, ${m}%, ${y}%, ${k}%)`;
  }, (s) => {
    const n = nums(s).map((v) => clamp(v, 0, 100) / 100);
    if (n.length < 4) return null;
    const [c, m, y, k] = n;
    return { r: Math.round(255 * (1 - c) * (1 - k)), g: Math.round(255 * (1 - m) * (1 - k)), b: Math.round(255 * (1 - y) * (1 - k)) };
  }],
  ["Android / .NET", () => `#FF${toHex(color).slice(1)}`, (s) => {
    const h = s.trim().replace(/^#|^0x/i, "");
    if (!/^[0-9a-f]{8}$/i.test(h)) return null;
    return { r: parseInt(h.slice(2, 4), 16), g: parseInt(h.slice(4, 6), 16), b: parseInt(h.slice(6, 8), 16) };
  }],
];

function renderColorFields() {
  const box = $("#color-fields");
  box.innerHTML = "";
  for (const [name] of COLOR_FIELDS) {
    const f = document.createElement("label");
    f.className = "num-field";
    f.dataset.name = name;
    f.innerHTML = `<span><em style="font-style:normal"></em><button class="copy"></button></span><input spellcheck="false" />`;
    f.querySelector("em").textContent = name;
    f.querySelector(".copy").textContent = t("copy");
    box.append(f);
  }
  box.oninput = (e) => {
    const field = e.target.closest(".num-field");
    const [, , parse] = COLOR_FIELDS.find((c) => c[0] === field.dataset.name);
    const rgb = parse(e.target.value);
    field.classList.toggle("bad", !rgb);
    if (rgb) setColor(rgb, field);
  };
  box.onclick = (e) => {
    const btn = e.target.closest(".copy");
    if (!btn) return;
    e.preventDefault();
    copyText(btn.closest(".num-field").querySelector("input").value, btn);
  };
  setColor(color);
}

function setColor(rgb, except) {
  Object.assign(color, { r: clamp(Math.round(rgb.r), 0, 255), g: clamp(Math.round(rgb.g), 0, 255), b: clamp(Math.round(rgb.b), 0, 255) });
  const hex = toHex(color);
  for (const f of document.querySelectorAll("#color-fields .num-field")) {
    if (f === except) continue;
    f.classList.remove("bad");
    f.querySelector("input").value = COLOR_FIELDS.find((c) => c[0] === f.dataset.name)[1]();
  }
  const sw = $("#swatch");
  sw.style.background = hex;
  sw.style.setProperty("--swatch", hex);
  $("#color-pick").value = hex.toLowerCase();

  const L = luminance(color);
  const cw = contrast(L, 1);
  const cb = contrast(L, 0);
  const grade = (c) => (c >= 7 ? "AAA" : c >= 4.5 ? "AA" : c >= 3 ? "AA Large" : "✕");
  $("#contrast").innerHTML = `
    <div style="background:#fff;color:${hex}">${t("contrastWhite")} <em>${cw.toFixed(2)} · ${grade(cw)}</em></div>
    <div style="background:#000;color:${hex}">${t("contrastBlack")} <em>${cb.toFixed(2)} · ${grade(cb)}</em></div>`;

  const { h, s } = rgbToHsl(color);
  const shades = $("#shades");
  shades.innerHTML = "";
  for (const l of [95, 90, 80, 70, 60, 50, 40, 30, 20, 12, 6]) {
    const c = hslToRgb(h, s, l);
    const b = document.createElement("button");
    const hx = toHex(c);
    b.style.background = hx;
    b.style.color = l > 55 ? "#0a0a12" : "#fff";
    b.textContent = hx.slice(1);
    b.title = hx;
    b.onclick = () => setColor(c);
    shades.append(b);
  }
}

/* ---------- shared ---------- */

async function copyText(text, btn) {
  try {
    await navigator.clipboard.writeText(text);
    btn.textContent = t("copied");
    setTimeout(() => (btn.textContent = t("copy")), 1200);
  } catch {}
}

export function initTools() {
  initUnits();
  initBase64();
  $("#color-pick").oninput = (e) => setColor(COLOR_FIELDS[0][2](e.target.value));
}

/** Re-renders everything with text in it (after a language change). */
export function refreshTools() {
  renderUnits();
  calcUnits("a");
  const decimal = document.querySelector('.num-field[data-key="decimal"] input')?.value ?? "255";
  renderNumbers();
  setNumber("decimal", decimal || "255");
  renderColorFields();
}
