// SPDX-License-Identifier: GPL-3.0-or-later
// Renders the lines of a recording list on a pool of workers (lines are independent).
import type { FromWorker, RenderSettings, ToWorker } from "./batch-types";

export interface Job {
  /** Position among the readable lines: numbers the WAV file and seeds its noise. */
  index: number;
  /** Line number in the list text. */
  lineNo: number;
  text: string;
}

export type Outcome =
  | { ok: true; job: Job; name: string; wav: Uint8Array; aliases: string[]; times: number[][] }
  | { ok: false; job: Job; message: string };

export interface RunOptions {
  jobs: Job[];
  settings: RenderSettings;
  workers: number;
  /** Speaker file (XML); empty for the bundled speaker. */
  speaker: string;
  /** Called after every change: lines done, all lines, the lines being rendered now. */
  onProgress(done: number, total: number, active: string[]): void;
  signal: AbortSignal;
}

export function runBatch(o: RunOptions): Promise<Outcome[]> {
  const total = o.jobs.length;
  const outcomes: Outcome[] = new Array(total);
  return new Promise((resolve, reject) => {
    if (total === 0) { resolve(outcomes); return; }
    if (o.signal.aborted) { reject(new DOMException("aborted", "AbortError")); return; }

    const pool: Worker[] = [];
    const active = new Map<Worker, string>();
    let next = 0;
    let done = 0;
    let over = false;

    const end = (err?: unknown) => {
      if (over) return;
      over = true;
      o.signal.removeEventListener("abort", onAbort);
      for (const w of pool) w.terminate();
      if (err) reject(err); else resolve(outcomes);
    };
    const onAbort = () => end(new DOMException("aborted", "AbortError"));
    o.signal.addEventListener("abort", onAbort);

    const report = () => o.onProgress(done, total, [...active.values()]);
    const feed = (w: Worker) => {
      if (next >= total) { active.delete(w); report(); return; }
      const pos = next++;
      const job = o.jobs[pos];
      active.set(w, job.text);
      const msg: ToWorker = { type: "job", id: pos, index: job.index, text: job.text, settings: o.settings };
      w.postMessage(msg);
      report();
    };

    const n = Math.max(1, Math.min(o.workers, total));
    for (let i = 0; i < n; i++) {
      const w = new Worker(new URL("./batch-worker.ts", import.meta.url), { type: "module" });
      pool.push(w);
      w.onmessage = (e: MessageEvent<FromWorker>) => {
        const m = e.data;
        if (m.type === "ready") { feed(w); return; }
        if (m.id < 0) { end(new Error(`音声エンジンを起動できませんでした: ${m.type === "fail" ? m.message : ""}`)); return; }
        const job = o.jobs[m.id];
        outcomes[m.id] = m.type === "done"
          ? { ok: true, job, name: m.name, wav: m.wav, aliases: m.aliases, times: m.times }
          : { ok: false, job, message: m.message };
        done++;
        if (done === total) { active.clear(); report(); end(); } else feed(w);
      };
      w.onerror = (ev) => end(new Error(`ワーカーが停止しました: ${ev.message || "原因不明"}`));
      const init: ToWorker = { type: "init", speaker: o.speaker };
      w.postMessage(init);
    }
    report();
  });
}
