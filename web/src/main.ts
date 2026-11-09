// SPDX-License-Identifier: GPL-3.0-or-later
import "./style.css";
import init, { version, lf_shape_from_rd, lf_preview } from "./wasm-pkg/vt2umora_wasm.js";
import { detectCapabilities } from "./capabilities";
import { drawSpectrum, drawWave } from "./plots";

const SAMPLE_RATE = 48000;
const WAVE_POINTS = 256;

interface State { rd: number; ra: number; rk: number; rg: number; breath: number; asp: number; f0: number }

interface Spec {
  key: keyof State;
  name: string;
  sub: string;
  min: number; max: number; step: number; digits: number; unit?: string;
}
interface Group { title: string; hint?: () => string; specs: Spec[] }

const GROUPS: Group[] = [
  { title: "声質", specs: [
    { key: "rd", name: "Rd", sub: "小さいほど張りのある声、大きいほど柔らかい声", min: 0.3, max: 2.7, step: 0.01, digits: 2 },
  ] },
  { title: "声門波の形", hint: () => (followsRd() ? "Rd に連動中" : "手動で調整中"), specs: [
    { key: "ra", name: "Ra", sub: "音響的な丸み・閉じ残し", min: 0.003, max: 0.2, step: 0.001, digits: 3 },
    { key: "rk", name: "Rk", sub: "立ち上がりと閉じの非対称性", min: 0.15, max: 0.8, step: 0.005, digits: 3 },
    { key: "rg", name: "Rg", sub: "開き始めの速さ(声門の共鳴比)", min: 0.5, max: 2.0, step: 0.005, digits: 3 },
  ] },
  { title: "息", specs: [
    { key: "breath", name: "Breath", sub: "肺の気圧", min: 0, max: 20, step: 0.1, digits: 1, unit: "cmH₂O" },
    { key: "asp", name: "Aspiration", sub: "息漏れノイズ", min: 0, max: 1, step: 0.01, digits: 2 },
  ] },
  { title: "ピッチ", specs: [
    { key: "f0", name: "F0", sub: "プレビューの基本周波数", min: 60, max: 600, step: 1, digits: 0, unit: "Hz" },
  ] },
];

const DEFAULTS = (): State => {
  const [ra, rk, rg] = lf_shape_from_rd(1.0);
  return { rd: 1.0, ra, rk, rg, breath: 8, asp: 0, f0: 120 };
};

let state: State;
const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const inputs = new Map<keyof State, { input: HTMLInputElement; val: HTMLElement; spec: Spec }>();
const hintEls: HTMLElement[] = [];

function followsRd(): boolean {
  const [ra, rk, rg] = lf_shape_from_rd(state.rd);
  return Math.abs(ra - state.ra) < 1e-6 && Math.abs(rk - state.rk) < 1e-6 && Math.abs(rg - state.rg) < 1e-6;
}

function fmt(spec: Spec, v: number) {
  return `${v.toFixed(spec.digits)}${spec.unit ? " " + spec.unit : ""}`;
}

function syncRow(key: keyof State) {
  const r = inputs.get(key)!;
  r.input.value = String(state[key]);
  const p = ((state[key] - r.spec.min) / (r.spec.max - r.spec.min)) * 100;
  r.input.style.setProperty("--p", `${Math.max(0, Math.min(100, p))}%`);
  r.val.textContent = fmt(r.spec, state[key]);
}
function syncAll() {
  (Object.keys(state) as (keyof State)[]).forEach(syncRow);
  GROUPS.forEach((g, i) => { if (g.hint) hintEls[i].textContent = g.hint(); });
}

function applyRd() {
  const [ra, rk, rg] = lf_shape_from_rd(state.rd);
  state.ra = ra; state.rk = rk; state.rg = rg;
}

function buildControls() {
  const host = $("controls");
  GROUPS.forEach((g, gi) => {
    const sec = document.createElement("section");
    sec.className = "group";
    const h = document.createElement("h2");
    h.textContent = g.title;
    const small = document.createElement("small");
    hintEls[gi] = small;
    h.appendChild(small);
    sec.appendChild(h);
    for (const spec of g.specs) {
      const row = document.createElement("label");
      row.className = "row";
      row.innerHTML = `<span class="name"><span>${spec.name}</span><small>${spec.sub}</small></span><span class="val"></span>`;
      const input = document.createElement("input");
      input.type = "range";
      input.min = String(spec.min); input.max = String(spec.max); input.step = String(spec.step);
      input.setAttribute("aria-label", `${spec.name} ${spec.sub}`);
      row.appendChild(input);
      sec.appendChild(row);
      inputs.set(spec.key, { input, val: row.querySelector(".val") as HTMLElement, spec });
      input.addEventListener("input", () => {
        state[spec.key] = Number(input.value);
        if (spec.key === "rd") applyRd();
        syncAll();
        schedule();
      });
    }
    host.appendChild(sec);
  });
}

// ---- preview ------------------------------------------------------------------------
let pending = false;
function schedule() {
  if (pending) return;
  pending = true;
  requestAnimationFrame(() => { pending = false; render(); });
}

function render() {
  const notice = $("notice");
  let pv;
  try {
    pv = lf_preview(state.f0, state.ra, state.rk, state.rg, state.breath, state.asp, SAMPLE_RATE, WAVE_POINTS);
  } catch {
    notice.hidden = false; // keep the last good drawing on screen
    return;
  }
  notice.hidden = true;
  const dflow = pv.dflow(), flow = pv.flow(), spectrumDb = pv.spectrum_db();
  const tilt = pv.tilt_db_per_oct, level = pv.tilt_level_1k_db, h1h2 = pv.h1_h2_db;
  const te = pv.te, tp = pv.tp, ta = pv.ta, voiced = pv.voiced, binHz = pv.bin_hz;
  pv.free();

  drawWave($("wave") as HTMLCanvasElement, { dflow: Float64Array.from(dflow), flow: Float64Array.from(flow), te, tp });
  drawSpectrum($("spec") as HTMLCanvasElement, { spectrumDb: Float64Array.from(spectrumDb), binHz, tiltDbPerOct: tilt, tiltLevel1kDb: level });

  const pct = (x: number) => `${(x * 100).toFixed(0)}%`;
  $("r-gain").innerHTML = voiced ? `Ee <b>−${(Math.max(...dflow.map((v) => -v))).toFixed(2)}</b>` : "<b>声帯振動なし</b>(気圧がしきい値未満)";
  $("r-times").innerHTML = `Tp <b>${pct(tp)}</b> Te <b>${pct(te)}</b> Ta <b>${pct(ta)}</b>`;
  $("r-tilt").innerHTML = voiced ? `傾き <b>${tilt.toFixed(1)} dB/oct</b>` : "";
  $("r-h1h2").innerHTML = voiced ? `H1−H2 <b>${h1h2.toFixed(1)} dB</b>` : "";
}

// ---- shell: nav, drawer -------------------------------------------------------------
const ICONS: Record<string, string> = {
  voice: '<path d="M3 12h2.5l2-6 3 12 3-9 2 5 1.5-2H21"/>',
  skeleton: '<path d="M7 4c0 5 1 7 5 8s5 3 5 8M7 4h6M17 20h-4"/>',
  consonant: '<path d="M4 17c3 0 4-10 8-10s5 10 8 10"/>',
  scrub: '<path d="M4 8h16M4 16h16M10 5v6M15 13v6"/>',
  export: '<path d="M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19h14"/>',
};
const NAV: { id: string; label: string; icon: string; ready: boolean }[] = [
  { id: "voice", label: "声質", icon: "voice", ready: true },
  { id: "skeleton", label: "骨格", icon: "skeleton", ready: false },
  { id: "consonant", label: "子音", icon: "consonant", ready: false },
  { id: "scrub", label: "スクラブ", icon: "scrub", ready: false },
  { id: "export", label: "書き出し", icon: "export", ready: false },
];

function buildNav() {
  const nav = $("nav");
  for (const n of NAV) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "nav-item";
    b.innerHTML = `<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${ICONS[n.icon]}</svg><span>${n.label}</span>${n.ready ? "" : "<small>準備中</small>"}`;
    if (n.ready) b.setAttribute("aria-current", "page"); else b.setAttribute("aria-disabled", "true");
    nav.appendChild(b);
  }
}

function setupDrawer() {
  const sidebar = $("sidebar"), scrim = $("scrim"), btn = $("menu");
  const set = (open: boolean) => {
    sidebar.classList.toggle("is-open", open);
    scrim.hidden = !open;
    btn.setAttribute("aria-expanded", String(open));
  };
  btn.addEventListener("click", () => set(!sidebar.classList.contains("is-open")));
  scrim.addEventListener("click", () => set(false));
  window.addEventListener("keydown", (e) => { if (e.key === "Escape") set(false); });
  window.matchMedia("(min-width: 769px)").addEventListener("change", (e) => { if (e.matches) set(false); });
}

async function main() {
  await init();
  state = DEFAULTS();
  buildNav();
  buildControls();
  setupDrawer();
  syncAll();

  const caps = detectCapabilities();
  $("env").textContent = caps.mode === "threaded" ? `高速モード · core v${version()}` : `標準モード · core v${version()}`;

  $("reset").addEventListener("click", () => { state = DEFAULTS(); syncAll(); schedule(); });
  $("fix").addEventListener("click", () => { applyRd(); syncAll(); schedule(); });
  new ResizeObserver(schedule).observe($("content"));
  render();
}

main().catch((e) => {
  document.body.style.overflow = "auto";
  document.body.textContent = `起動に失敗しました: ${e}`;
});
