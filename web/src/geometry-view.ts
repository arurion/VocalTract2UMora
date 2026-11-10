// SPDX-License-Identifier: GPL-3.0-or-later
import { geometry_info } from "./wasm-pkg/vt2umora_wasm.js";
import { buildSlider, group, type Slider, type View } from "./controls";
import { LIMITS, PRESETS, defaultGeometry, downloadJson, parseGeometry, type Geometry } from "./geometry";
import { drawTract } from "./plots";

export function createGeometryView(onChange: () => void): View {
  const el = document.getElementById("view-geometry")!;
  const $ = (id: string) => document.getElementById(id)!;
  let g: Geometry = defaultGeometry();

  const sliders: Record<"len" | "ratio" | "nasal", Slider> = {} as never;
  const status = document.createElement("p");
  status.className = "status"; status.setAttribute("role", "status");
  const nameInput = document.createElement("input");

  const sync = () => {
    sliders.len.set(g.tractLengthCm);
    sliders.ratio.set(g.oralRatio);
    sliders.nasal.set(g.nasalVolumeScale);
    nameInput.value = g.name;
  };
  const say = (msg: string, error = false) => { status.textContent = msg; status.classList.toggle("is-error", error); };

  const host = $("g-controls");

  const shape = group("声道");
  sliders.len = buildSlider({ name: "声道長", sub: "声門から唇まで", ...LIMITS.tractLengthCm, step: 0.1, digits: 1, unit: "cm" }, g.tractLengthCm,
    (v) => { g.tractLengthCm = v; onChange(); });
  sliders.ratio = buildSlider({ name: "口腔の割合", sub: "全長のうち口腔が占める割合(残りは咽頭)", ...LIMITS.oralRatio, step: 0.01, digits: 2 }, g.oralRatio,
    (v) => { g.oralRatio = v; onChange(); });
  shape.sec.append(sliders.len.row, sliders.ratio.row);

  const nasal = group("鼻腔");
  sliders.nasal = buildSlider({ name: "鼻腔容積", sub: "基準を 1.0 とした倍率", ...LIMITS.nasalVolumeScale, step: 0.05, digits: 2, unit: "×" }, g.nasalVolumeScale,
    (v) => { g.nasalVolumeScale = v; onChange(); });
  nasal.sec.append(sliders.nasal.row);

  const presets = group("プリセット");
  presets.hint.textContent = "目安の値";
  const prow = document.createElement("div");
  prow.className = "btn-row";
  for (const p of PRESETS) {
    const b = document.createElement("button");
    b.type = "button"; b.className = "btn"; b.textContent = p.label;
    b.addEventListener("click", () => { g = { ...p.value }; sync(); say(""); onChange(); });
    prow.appendChild(b);
  }
  presets.sec.appendChild(prow);

  const file = group("ファイル");
  file.hint.textContent = "JSON";
  const nameRow = document.createElement("label");
  nameRow.className = "row";
  nameRow.innerHTML = '<span class="name"><span>話者名</span><small>書き出すファイル名に使います</small></span>';
  nameInput.type = "text"; nameInput.className = "field"; nameInput.maxLength = 40; nameInput.value = g.name;
  nameInput.setAttribute("aria-label", "話者名");
  nameInput.addEventListener("input", () => { g.name = nameInput.value; });
  nameRow.appendChild(nameInput);

  const frow = document.createElement("div");
  frow.className = "btn-row";
  const exp = document.createElement("button");
  exp.type = "button"; exp.className = "btn"; exp.textContent = "書き出し";
  exp.addEventListener("click", () => { downloadJson({ ...g, name: g.name.trim() || "無題" }); say("書き出しました。"); });
  const imp = document.createElement("button");
  imp.type = "button"; imp.className = "btn"; imp.textContent = "読み込み";
  const picker = document.createElement("input");
  picker.type = "file"; picker.accept = ".json,application/json"; picker.hidden = true;
  imp.addEventListener("click", () => picker.click());
  picker.addEventListener("change", async () => {
    const f = picker.files?.[0];
    picker.value = "";
    if (!f) return;
    const res = parseGeometry(await f.text());
    if (!res.ok) { say(res.error, true); return; }
    g = res.value; sync(); say(`「${g.name}」を読み込みました。`); onChange();
  });
  frow.append(exp, imp, picker);
  file.sec.append(nameRow, frow, status);

  host.append(shape.sec, nasal.sec, presets.sec, file.sec);

  const render = () => {
    if (el.hidden) return;
    const [len, ratio, nas, oral, phar, f1, f2, f3] = geometry_info(g.tractLengthCm, g.oralRatio, g.nasalVolumeScale);
    drawTract($("tract") as HTMLCanvasElement, { lengthCm: len, oralRatio: ratio, nasalScale: nas });
    $("g-lens").innerHTML = `全長 <b>${len.toFixed(1)} cm</b> 咽頭 <b>${phar.toFixed(1)}</b> / 口腔 <b>${oral.toFixed(1)}</b>`;
    $("g-formants").innerHTML = `一様管の共鳴 F1 <b>${f1.toFixed(0)}</b> F2 <b>${f2.toFixed(0)}</b> F3 <b>${f3.toFixed(0)} Hz</b>`;
  };

  return {
    id: "geometry", title: "骨格エディタ", subtitle: "話者の声道の大きさと比率を決める", el, render,
    reset() { g = defaultGeometry(); sync(); say(""); },
  };
}
