// SPDX-License-Identifier: GPL-3.0-or-later
// One worker of the batch pool: owns a synthesiser (its own Wasm instance) and renders whole
// lines of a recording list. Plain postMessage, so it also works without shared memory.
import init, { BatchConfig, Engine } from "./wasm-pkg/vt2umora_wasm.js";
import type { FromWorker, RenderSettings, ToWorker } from "./batch-types";

const ctx = self as unknown as Worker;
const post = (m: FromWorker, transfer: Transferable[] = []) => ctx.postMessage(m, transfer);

let engine: Engine | null = null;
let ready: Promise<void> | null = null;

function render(id: number, index: number, text: string, s: RenderSettings) {
  const cfg = new BatchConfig(s.style);
  try {
    cfg.set_f0_hz(s.f0Hz);
    cfg.set_vowel_hold_s(s.vowelHoldS);
    cfg.set_geometry_stride(s.geometryStride);
    cfg.set_sample_rate(s.sampleRate);
    cfg.set_noise_seed(s.seed);
    const line = engine!.render_line(text, index, cfg);
    try {
      const aliases: string[] = [];
      const times: number[][] = [];
      for (let k = 0; k < line.entry_count; k++) {
        aliases.push(line.entry_alias(k));
        times.push(Array.from(line.entry_times(k)));
      }
      const wav = line.wav();
      post({ type: "done", id, name: line.wav_name, wav, aliases, times }, [wav.buffer]);
    } finally {
      line.free();
    }
  } finally {
    cfg.free();
  }
}

ctx.onmessage = (e: MessageEvent<ToWorker>) => {
  const m = e.data;
  if (m.type === "init") {
    ready = init()
      .then(() => { engine = new Engine(m.speaker); post({ type: "ready" }); })
      .catch((err) => post({ type: "fail", id: -1, message: String(err instanceof Error ? err.message : err) }));
    return;
  }
  ready?.then(() => {
    try {
      render(m.id, m.index, m.text, m.settings);
    } catch (err) {
      post({ type: "fail", id: m.id, message: err instanceof Error ? err.message : String(err) });
    }
  });
};
