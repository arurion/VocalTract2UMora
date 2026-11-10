// SPDX-License-Identifier: GPL-3.0-or-later
import { lf_shape_from_rd, lf_preview } from "./wasm-pkg/vt2umora_wasm.js";
import { buildSlider, group, type Slider, type Spec, type View } from "./controls";
import { drawSpectrum, drawWave } from "./plots";

const SAMPLE_RATE = 48000;
const WAVE_POINTS = 256;

interface State { rd: number; ra: number; rk: number; rg: number; breath: number; asp: number; f0: number }
type Key = keyof State;
interface Row extends Spec { key: Key }
interface Group { title: string; hint?: () => string; rows: Row[] }

const GROUPS: Group[] = [
  { title: "声質", rows: [
    { key: "rd", name: "Rd", sub: "小さいほど張りのある声、大きいほど柔らかい声", min: 0.3, max: 2.7, step: 0.01, digits: 2 },
  ] },
  { title: "声門波の形", hint: () => (followsRd() ? "Rd に連動中" : "手動で調整中"), rows: [
    { key: "ra", name: "Ra", sub: "音響的な丸み・閉じ残し", min: 0.003, max: 0.2, step: 0.001, digits: 3 },
    { key: "rk", name: "Rk", sub: "立ち上がりと閉じの非対称性", min: 0.15, max: 0.8, step: 0.005, digits: 3 },
    { key: "rg", name: "Rg", sub: "開き始めの速さ(声門の共鳴比)", min: 0.5, max: 2.0, step: 0.005, digits: 3 },
  ] },
  { title: "息", rows: [
    { key: "breath", name: "Breath", sub: "肺の気圧", min: 0, max: 20, step: 0.1, digits: 1, unit: "cmH₂O" },
    { key: "asp", name: "Aspiration", sub: "息漏れノイズ", min: 0, max: 1, step: 0.01, digits: 2 },
  ] },
  { title: "ピッチ", rows: [
    { key: "f0", name: "F0", sub: "プレビューの基本周波数", min: 60, max: 600, step: 1, digits: 0, unit: "Hz" },
  ] },
];

const defaults = (): State => {
  const [ra, rk, rg] = lf_shape_from_rd(1.0);
  return { rd: 1.0, ra, rk, rg, breath: 8, asp: 0, f0: 120 };
};

let state: State = defaults();
const sliders = new Map<Key, Slider>();
const hints: (() => void)[] = [];

function followsRd(): boolean {
  const [ra, rk, rg] = lf_shape_from_rd(state.rd);
  return Math.abs(ra - state.ra) < 1e-6 && Math.abs(rk - state.rk) < 1e-6 && Math.abs(rg - state.rg) < 1e-6;
}
function syncAll() {
  (Object.keys(state) as Key[]).forEach((k) => sliders.get(k)!.set(state[k]));
  hints.forEach((h) => h());
}
function applyRd() {
  const [ra, rk, rg] = lf_shape_from_rd(state.rd);
  state.ra = ra; state.rk = rk; state.rg = rg;
}

export function createVoiceView(onChange: () => void): View {
  const el = document.getElementById("view-voice")!;
  const $ = (id: string) => document.getElementById(id)!;

  const host = $("controls");
  for (const g of GROUPS) {
    const { sec, hint } = group(g.title);
    if (g.hint) { const h = g.hint; hints.push(() => { hint.textContent = h(); }); }
    for (const r of g.rows) {
      const s = buildSlider(r, state[r.key], (v) => {
        state[r.key] = v;
        if (r.key === "rd") applyRd();
        syncAll();
        onChange();
      });
      sliders.set(r.key, s);
      sec.appendChild(s.row);
    }
    host.appendChild(sec);
  }
  syncAll();
  $("fix").addEventListener("click", () => { applyRd(); syncAll(); onChange(); });

  const render = () => {
    if (el.hidden) return;
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
    $("r-gain").innerHTML = voiced ? `Ee <b>−${Math.max(...dflow.map((v) => -v)).toFixed(2)}</b>` : "<b>声帯振動なし</b>(気圧がしきい値未満)";
    $("r-times").innerHTML = `Tp <b>${pct(tp)}</b> Te <b>${pct(te)}</b> Ta <b>${pct(ta)}</b>`;
    $("r-tilt").innerHTML = voiced ? `傾き <b>${tilt.toFixed(1)} dB/oct</b>` : "";
    $("r-h1h2").innerHTML = voiced ? `H1−H2 <b>${h1h2.toFixed(1)} dB</b>` : "";
  };

  return {
    id: "voice", title: "声質エディタ", subtitle: "声帯が作る音源(LF声門波)を整える", el, render,
    reset() { state = defaults(); syncAll(); },
  };
}
