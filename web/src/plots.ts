// SPDX-License-Identifier: GPL-3.0-or-later
// Canvas plots for the voice editor. Pure drawing; no state.

export interface WaveData {
  dflow: Float64Array;
  flow: Float64Array;
  te: number;
  tp: number;
}
export interface SpecData {
  spectrumDb: Float64Array;
  binHz: number;
  tiltDbPerOct: number;
  tiltLevel1kDb: number;
}

interface Colors { line: string; soft: string; accent: string; axis: string; guide: string; text: string }

function colors(): Colors {
  const s = getComputedStyle(document.documentElement);
  const g = (n: string) => s.getPropertyValue(n).trim();
  return { line: g("--plot-line"), soft: g("--plot-soft"), accent: g("--plot-accent"), axis: g("--plot-axis"), guide: g("--plot-guide"), text: g("--text") };
}

/** Keeps the canvas bitmap in sync with its CSS size (devicePixelRatio aware). */
export function fitCanvas(c: HTMLCanvasElement): CanvasRenderingContext2D {
  const dpr = Math.max(1, Math.min(3, window.devicePixelRatio || 1));
  const w = Math.max(1, Math.round(c.clientWidth * dpr));
  const h = Math.max(1, Math.round(c.clientHeight * dpr));
  if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
  const ctx = c.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return ctx;
}

const PAD = { l: 44, r: 12, t: 10, b: 24 };
const FONT = '11px "Hiragino Sans","Noto Sans JP","Yu Gothic UI",system-ui,sans-serif';

export function drawWave(c: HTMLCanvasElement, d: WaveData): void {
  const ctx = fitCanvas(c);
  const col = colors();
  const W = c.clientWidth, H = c.clientHeight;
  ctx.clearRect(0, 0, W, H);
  const x0 = PAD.l, x1 = W - PAD.r, y0 = PAD.t, y1 = H - PAD.b;

  let lo = 0, hi = 0;
  for (const v of d.dflow) { lo = Math.min(lo, v); hi = Math.max(hi, v); }
  const amp = Math.max(lo === 0 && hi === 0 ? 1 : 0, -lo, hi * 1.0);
  const top = Math.max(hi * 1.15, amp * 0.25), bot = Math.min(lo * 1.15, -amp * 0.25);
  const yOf = (v: number) => y1 - ((v - bot) / (top - bot)) * (y1 - y0);
  const xOf = (t: number) => x0 + t * (x1 - x0);

  ctx.font = FONT; ctx.textBaseline = "middle";
  // zero line + value ticks
  ctx.strokeStyle = col.guide; ctx.lineWidth = 1;
  ctx.beginPath(); ctx.moveTo(x0, yOf(0)); ctx.lineTo(x1, yOf(0)); ctx.stroke();
  ctx.fillStyle = col.axis; ctx.textAlign = "right";
  for (const v of niceTicks(bot, top, 4)) { ctx.fillText(fmtTick(v), x0 - 6, yOf(v)); }

  // Tp / Te markers
  ctx.setLineDash([3, 3]); ctx.strokeStyle = col.axis; ctx.textAlign = "center"; ctx.textBaseline = "alphabetic";
  for (const [t, label] of [[d.tp, "Tp"], [d.te, "Te"]] as const) {
    ctx.beginPath(); ctx.moveTo(xOf(t), y0); ctx.lineTo(xOf(t), y1); ctx.stroke();
    ctx.fillText(label, xOf(t), H - 6);
  }
  ctx.setLineDash([]);
  ctx.fillText("0", x0, H - 6); ctx.fillText("T0", x1, H - 6);

  // Ug (reference, normalised to its own peak) behind dUg/dt
  let fmax = 0; for (const v of d.flow) fmax = Math.max(fmax, Math.abs(v));
  if (fmax > 0) {
    ctx.strokeStyle = col.soft; ctx.lineWidth = 1.5; ctx.beginPath();
    const k = (top * 0.9) / fmax;
    d.flow.forEach((v, i) => { const x = xOf(i / d.flow.length), y = yOf(v * k); i ? ctx.lineTo(x, y) : ctx.moveTo(x, y); });
    ctx.stroke();
  }
  // dUg/dt
  ctx.strokeStyle = col.line; ctx.lineWidth = 2; ctx.lineJoin = "round"; ctx.beginPath();
  const n = d.dflow.length;
  for (let i = 0; i <= n; i++) { const x = xOf(i / n), y = yOf(d.dflow[i % n]); i ? ctx.lineTo(x, y) : ctx.moveTo(x, y); }
  ctx.stroke();
}

const F_MIN = 50, F_MAX = 8000;

export function drawSpectrum(c: HTMLCanvasElement, d: SpecData): void {
  const ctx = fitCanvas(c);
  const col = colors();
  const W = c.clientWidth, H = c.clientHeight;
  ctx.clearRect(0, 0, W, H);
  const x0 = PAD.l, x1 = W - PAD.r, y0 = PAD.t, y1 = H - PAD.b;
  const lr = Math.log(F_MAX / F_MIN);
  const xOf = (f: number) => x0 + (Math.log(f / F_MIN) / lr) * (x1 - x0);

  let mx = -Infinity;
  for (let k = 1; k < d.spectrumDb.length; k++) if (k * d.binHz <= F_MAX) mx = Math.max(mx, d.spectrumDb[k]);
  const top = Math.ceil((mx + 4) / 10) * 10, range = 90, bot = top - range;
  const yOf = (db: number) => y1 - ((Math.max(bot, Math.min(top, db)) - bot) / range) * (y1 - y0);

  ctx.font = FONT; ctx.fillStyle = col.axis; ctx.strokeStyle = col.guide; ctx.lineWidth = 1;
  ctx.textAlign = "right"; ctx.textBaseline = "middle";
  for (let db = top; db >= bot; db -= 20) { ctx.beginPath(); ctx.moveTo(x0, yOf(db)); ctx.lineTo(x1, yOf(db)); ctx.stroke(); ctx.fillText(String(db), x0 - 6, yOf(db)); }
  ctx.textAlign = "center"; ctx.textBaseline = "alphabetic";
  for (const f of [100, 200, 500, 1000, 2000, 5000]) { ctx.fillText(f >= 1000 ? `${f / 1000}k` : String(f), xOf(f), H - 6); }
  ctx.textAlign = "left"; ctx.fillText("dB", 6, y0 + 8);

  // spectrum: per-pixel max keeps harmonics crisp on the log axis
  const px = Math.max(1, Math.floor(x1 - x0));
  const cols = new Float64Array(px).fill(-Infinity);
  for (let k = 1; k < d.spectrumDb.length; k++) {
    const f = k * d.binHz; if (f < F_MIN) continue; if (f > F_MAX) break;
    const i = Math.min(px - 1, Math.floor(((xOf(f) - x0) / (x1 - x0)) * px));
    if (d.spectrumDb[k] > cols[i]) cols[i] = d.spectrumDb[k];
  }
  ctx.strokeStyle = col.line; ctx.lineWidth = 1.5; ctx.lineJoin = "round"; ctx.beginPath();
  let started = false;
  for (let i = 0; i < px; i++) { if (cols[i] === -Infinity) continue; const x = x0 + i, y = yOf(cols[i]); started ? ctx.lineTo(x, y) : ctx.moveTo(x, y); started = true; }
  ctx.stroke();

  // spectral tilt: straight on a log-frequency axis
  if (Number.isFinite(d.tiltDbPerOct)) {
    const at = (f: number) => d.tiltLevel1kDb + d.tiltDbPerOct * Math.log2(f / 1000);
    ctx.strokeStyle = col.accent; ctx.lineWidth = 1.75; ctx.setLineDash([6, 4]);
    ctx.beginPath(); ctx.moveTo(xOf(100), yOf(at(100))); ctx.lineTo(xOf(5000), yOf(at(5000))); ctx.stroke(); ctx.setLineDash([]);
  }
}

function niceTicks(lo: number, hi: number, count: number): number[] {
  const span = hi - lo || 1, raw = span / count;
  const mag = Math.pow(10, Math.floor(Math.log10(raw)));
  const step = [1, 2, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? raw;
  const out: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + 1e-9; v += step) out.push(Math.abs(v) < 1e-12 ? 0 : v);
  return out;
}
const fmtTick = (v: number) => (Math.abs(v) >= 10 ? v.toFixed(0) : Math.abs(v) >= 1 ? v.toFixed(1) : v.toFixed(2));
