// SPDX-License-Identifier: GPL-3.0-or-later
// Shared slider row used by every editor view.

export interface Spec {
  name: string;
  sub: string;
  min: number; max: number; step: number; digits: number; unit?: string;
}

export interface Slider {
  row: HTMLElement;
  set(v: number): void;
}

export const fmt = (spec: Spec, v: number) => `${v.toFixed(spec.digits)}${spec.unit ? " " + spec.unit : ""}`;

export function buildSlider(spec: Spec, initial: number, onInput: (v: number) => void): Slider {
  const row = document.createElement("label");
  row.className = "row";
  row.innerHTML = `<span class="name"><span>${spec.name}</span><small>${spec.sub}</small></span><span class="val"></span>`;
  const val = row.querySelector(".val") as HTMLElement;
  const input = document.createElement("input");
  input.type = "range";
  input.min = String(spec.min); input.max = String(spec.max); input.step = String(spec.step);
  input.setAttribute("aria-label", `${spec.name} ${spec.sub}`);
  row.appendChild(input);

  const set = (v: number) => {
    input.value = String(v);
    const p = ((v - spec.min) / (spec.max - spec.min)) * 100;
    input.style.setProperty("--p", `${Math.max(0, Math.min(100, p))}%`);
    val.textContent = fmt(spec, v);
  };
  input.addEventListener("input", () => { const v = Number(input.value); set(v); onInput(v); });
  set(initial);
  return { row, set };
}

export function group(title: string): { sec: HTMLElement; hint: HTMLElement } {
  const sec = document.createElement("section");
  sec.className = "group";
  const h = document.createElement("h2");
  h.textContent = title;
  const hint = document.createElement("small");
  h.appendChild(hint);
  sec.appendChild(h);
  return { sec, hint };
}

export interface View {
  id: string;
  title: string;
  subtitle: string;
  el: HTMLElement;
  /** Draws the view (called when shown and whenever the layout changes). */
  render(): void;
  reset(): void;
}
