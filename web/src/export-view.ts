// SPDX-License-Identifier: GPL-3.0-or-later
import { builtin_list, new_assembler, plan_list } from "./wasm-pkg/vt2umora_wasm.js";
import { buildSlider, group, type Slider, type View } from "./controls";
import { runBatch, type Job, type Outcome } from "./batch-runner";
import type { RenderSettings, Style } from "./batch-types";

const STYLES: { id: Style; label: string; example: string; hint: string }[] = [
  { id: "single", label: "単独音", example: "か", hint: "かな 1 つにつき 1 つの音。先頭から切り出す一番素朴な形式" },
  { id: "vcv", label: "連続音", example: "あ か", hint: "前の母音を含む音(あ か、ん ま)と、語頭の音(- か)" },
  { id: "cvvc", label: "CVVC", example: "か + a k", hint: "子音+母音(か)と、母音から子音への音(a k)の 2 種類" },
];
const SPEEDS: { id: string; label: string; example: string; stride: number }[] = [
  { id: "std", label: "標準", example: "ほぼ同じ精度で速い", stride: 2 },
  { id: "fine", label: "高精度", example: "約 1.3 倍の時間", stride: 1 },
  { id: "fast", label: "高速", example: "oto が数 ms ずれうる", stride: 4 },
];

const fmtDuration = (s: number) => {
  const t = Math.max(0, Math.round(s));
  return t >= 60 ? `${Math.floor(t / 60)}分${String(t % 60).padStart(2, "0")}秒` : `${t}秒`;
};
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

export function createExportView(): View {
  const el = document.getElementById("view-export")!;
  const $ = (id: string) => document.getElementById(id)!;
  const cores = Math.max(2, Math.min(16, navigator.hardwareConcurrency || 4));
  const defaults = () => ({ style: "vcv" as Style, f0: 120, hold: 0.4, speed: "std", workers: Math.max(1, Math.min(6, cores - 1)), name: "VT2UMora" });
  let st = defaults();
  let running: AbortController | null = null;
  let zipUrl: string | null = null;
  let zipName = "";

  const list = $("e-list") as HTMLTextAreaElement;
  const info = $("e-list-info");
  const issues = $("e-issues");
  const bar = $("e-bar");
  const fill = $("e-bar-fill");

  // ---- controls ------------------------------------------------------------------------------
  const host = $("e-controls");

  const segmented = <T extends string>(items: { id: T; label: string; example: string }[], get: () => T, set: (v: T) => void, aria: string) => {
    const wrap = document.createElement("div");
    wrap.className = "seg"; wrap.setAttribute("role", "radiogroup"); wrap.setAttribute("aria-label", aria);
    const buttons = items.map((it) => {
      const b = document.createElement("button");
      b.type = "button"; b.setAttribute("role", "radio");
      b.innerHTML = `<span>${esc(it.label)}</span><small>${esc(it.example)}</small>`;
      b.addEventListener("click", () => { set(it.id); paint(); });
      wrap.appendChild(b);
      return { b, id: it.id };
    });
    const paint = () => buttons.forEach(({ b, id }) => b.setAttribute("aria-checked", String(id === get())));
    paint();
    return { wrap, paint };
  };

  const fmtGroup = group("形式");
  const styleHint = document.createElement("p");
  styleHint.className = "status";
  const styleSeg = segmented(STYLES, () => st.style, (v) => { st.style = v; hint(); }, "音源の形式");
  const hint = () => { styleHint.textContent = STYLES.find((s) => s.id === st.style)!.hint; };
  hint();
  fmtGroup.sec.append(styleSeg.wrap, styleHint);

  const voice = group("声");
  const sF0: Slider = buildSlider({ name: "基本周波数", sub: "録音の音の高さ(UTAU の基準音)", min: 70, max: 300, step: 1, digits: 0, unit: "Hz" }, st.f0, (v) => { st.f0 = v; });
  const sHold: Slider = buildSlider({ name: "母音の長さ", sub: "各音を保つ時間", min: 0.25, max: 0.8, step: 0.05, digits: 2, unit: "秒" }, st.hold, (v) => { st.hold = v; });
  voice.sec.append(sF0.row, sHold.row);

  const perf = group("速さ");
  const speedSeg = segmented(SPEEDS, () => st.speed, (v) => { st.speed = v; }, "速さと精度");
  const sWorkers: Slider = buildSlider({ name: "同時に生成する行数", sub: `この端末は ${cores} 並列まで`, min: 1, max: cores, step: 1, digits: 0, unit: "行" }, st.workers, (v) => { st.workers = v; });
  perf.sec.append(speedSeg.wrap, sWorkers.row);

  const out = group("出力");
  const nameRow = document.createElement("label");
  nameRow.className = "row";
  nameRow.innerHTML = '<span class="name"><span>音源名</span><small>character.txt と ZIP のファイル名に使います</small></span>';
  const nameInput = document.createElement("input");
  nameInput.type = "text"; nameInput.className = "field"; nameInput.maxLength = 40; nameInput.value = st.name;
  nameInput.setAttribute("aria-label", "音源名");
  nameInput.addEventListener("input", () => { st.name = nameInput.value; });
  nameRow.appendChild(nameInput);

  const go = document.createElement("button");
  go.type = "button"; go.className = "btn primary"; go.textContent = "音源を生成";
  const dl = document.createElement("button");
  dl.type = "button"; dl.className = "btn"; dl.textContent = "ZIP をダウンロード"; dl.disabled = true;
  const brow = document.createElement("div");
  brow.className = "btn-row"; brow.append(go, dl);
  const status = document.createElement("p");
  status.className = "status"; status.setAttribute("role", "status");
  out.sec.append(nameRow, brow, status);
  host.append(fmtGroup.sec, voice.sec, perf.sec, out.sec);

  const say = (msg: string, error = false) => { status.textContent = msg; status.classList.toggle("is-error", error); };

  // ---- the list ------------------------------------------------------------------------------
  function readList() {
    const plan = plan_list(list.value);
    const jobs: Job[] = [];
    const bad: string[] = [];
    let morae = 0;
    for (let i = 0; i < plan.len; i++) {
      const err = plan.error(i);
      if (err) bad.push(`${plan.line_no(i)}行目「${plan.text(i)}」: ${err}`);
      else { jobs.push({ index: jobs.length, lineNo: plan.line_no(i), text: plan.text(i) }); morae += plan.mora_count(i); }
    }
    plan.free();
    return { jobs, bad, morae };
  }

  const refreshList = () => {
    if (running) return;
    const { jobs, bad, morae } = readList();
    info.textContent = `${jobs.length} 行を読めました(約 ${fmtDuration(morae * 0.5)} の音声)` + (bad.length ? ` · 読めない行 ${bad.length}` : "");
    issues.innerHTML = bad.slice(0, 3).map((b) => `<p>${esc(b)}</p>`).join("") + (bad.length > 3 ? `<p>ほか ${bad.length - 3} 行</p>` : "");
    setProgress(0, 0, jobs.length ? `${jobs.length} 行を生成できます` : "生成できる行がありません", "");
  };
  list.addEventListener("input", refreshList);

  const picker = document.createElement("input");
  picker.type = "file"; picker.accept = ".txt,text/plain"; picker.hidden = true;
  $("e-load").addEventListener("click", () => picker.click());
  picker.addEventListener("change", async () => {
    const f = picker.files?.[0]; picker.value = "";
    if (!f || running) return;
    list.value = await f.text(); refreshList();
  });
  $("e-builtin").addEventListener("click", () => { if (!running) { list.value = builtin_list(); refreshList(); } });
  el.appendChild(picker);

  // ---- progress ------------------------------------------------------------------------------
  function setProgress(done: number, total: number, title: string, line: string) {
    const p = total ? done / total : 0;
    fill.style.width = `${(p * 100).toFixed(1)}%`;
    bar.setAttribute("aria-valuenow", String(Math.round(p * 100)));
    $("e-p-title").textContent = title;
    $("e-p-count").textContent = total ? `${done} / ${total} 行` : "";
    $("e-p-line").textContent = line;
  }

  function setRunning(on: boolean) {
    el.classList.toggle("is-running", on);
    list.readOnly = on;
    for (const b of [$("e-load"), $("e-builtin")]) (b as HTMLButtonElement).disabled = on;
    nameInput.disabled = on;
    go.textContent = on ? "中止" : "音源を生成";
  }

  // ---- result --------------------------------------------------------------------------------
  function showOto(text: string) {
    const rows = text.split("\n").filter(Boolean);
    const shown = rows.slice(0, 300).map((r) => {
      const eq = r.indexOf("=");
      const wav = r.slice(0, eq);
      const [alias, , cons, , pre, ovl] = r.slice(eq + 1).split(",");
      return `<div class="oto-row"><b>${esc(alias)}</b><span>${pre}</span><span>${ovl}</span><span>${cons}</span><span class="wavname">${esc(wav)}</span></div>`;
    });
    $("e-oto-body").innerHTML = shown.join("") + (rows.length > 300 ? `<p class="status">ほか ${rows.length - 300} 件</p>` : "");
    $("e-oto").hidden = rows.length === 0;
    $("e-oto-info").textContent = `${rows.length} 件`;
  }

  async function generate() {
    const { jobs, bad } = readList();
    if (!jobs.length) { say("読める行がありません。リストを確認してください。", true); return; }
    const settings: RenderSettings = {
      style: st.style, f0Hz: st.f0, vowelHoldS: st.hold,
      geometryStride: SPEEDS.find((s) => s.id === st.speed)!.stride, sampleRate: 44100, seed: 1,
    };
    running = new AbortController();
    setRunning(true); say(""); dl.disabled = true; $("e-oto").hidden = true;
    const t0 = performance.now();
    try {
      const outcomes: Outcome[] = await runBatch({
        jobs, settings, workers: st.workers, speaker: "", signal: running.signal,
        onProgress(done, total, active) {
          const el_ = (performance.now() - t0) / 1000;
          const eta = done > 0 ? ` · あと約 ${fmtDuration((el_ / done) * (total - done))}` : "";
          setProgress(done, total, done === total ? "まとめています" : "生成しています", `${active.slice(0, 3).join("   ")}${active.length > 3 ? " …" : ""}${eta}`);
        },
      });
      const asm = new_assembler();
      for (const b of bad) asm.add_error(b);
      for (const o of outcomes) {
        if (o.ok) {
          const idx = asm.add_wav(o.name, o.wav);
          o.aliases.forEach((alias, k) => asm.add_entry(idx, alias, Float64Array.from(o.times[k])));
        } else {
          asm.add_error(`${o.job.lineNo}行目「${o.job.text}」: ${o.message}`);
        }
      }
      const name = st.name.trim() || "VT2UMora";
      asm.finish(name);
      const zip = asm.zip();
      if (zipUrl) URL.revokeObjectURL(zipUrl);
      zipUrl = URL.createObjectURL(new Blob([zip as BlobPart], { type: "application/zip" }));
      zipName = `${name}.zip`;
      showOto(asm.oto_text());
      const errs = asm.errors();
      const secs = (performance.now() - t0) / 1000;
      issues.innerHTML = errs.slice(0, 3).map((b) => `<p>${esc(b)}</p>`).join("") + (errs.length > 3 ? `<p>ほか ${errs.length - 3} 行</p>` : "");
      setProgress(jobs.length, jobs.length, "完了", `${fmtDuration(secs)}かかりました`);
      say(`oto ${asm.entries} 件 · WAV ${asm.files} 本 · ${(zip.length / 1048576).toFixed(1)} MB` + (errs.length ? ` · 読めなかった行 ${errs.length}` : ""));
      dl.disabled = false;
      asm.free();
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") { setProgress(0, 0, "中止しました", ""); say(""); }
      else { setProgress(0, 0, "生成できませんでした", ""); say(String(e instanceof Error ? e.message : e), true); }
    } finally {
      running = null; setRunning(false);
    }
  }

  go.addEventListener("click", () => { if (running) running.abort(); else void generate(); });
  dl.addEventListener("click", () => {
    if (!zipUrl) return;
    const a = document.createElement("a");
    a.href = zipUrl; a.download = zipName; document.body.appendChild(a); a.click(); a.remove();
  });

  list.value = builtin_list();
  refreshList();

  return {
    id: "export", title: "音源の書き出し", subtitle: "録音リストから UTAU 音源(WAV と oto.ini)を作る", el,
    render() { /* static layout: nothing depends on the size */ },
    reset() {
      if (running) return;
      st = defaults();
      sF0.set(st.f0); sHold.set(st.hold); sWorkers.set(st.workers); nameInput.value = st.name;
      styleSeg.paint(); speedSeg.paint(); hint();
      list.value = builtin_list(); refreshList(); say("");
    },
  };
}
