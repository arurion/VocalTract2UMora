// SPDX-License-Identifier: GPL-3.0-or-later
import "./style.css";
import init, { version } from "./wasm-pkg/vt2umora_wasm.js";
import { detectCapabilities } from "./capabilities";
import type { View } from "./controls";
import { createGeometryView } from "./geometry-view";
import { createVoiceView } from "./voice-view";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const ICONS: Record<string, string> = {
  voice: '<path d="M3 12h2.5l2-6 3 12 3-9 2 5 1.5-2H21"/>',
  geometry: '<path d="M7 4c0 5 1 7 5 8s5 3 5 8M7 4h6M17 20h-4"/>',
  consonant: '<path d="M4 17c3 0 4-10 8-10s5 10 8 10"/>',
  scrub: '<path d="M4 8h16M4 16h16M10 5v6M15 13v6"/>',
  export: '<path d="M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19h14"/>',
};
const NAV: { id: string; label: string; ready: boolean }[] = [
  { id: "voice", label: "声質", ready: true },
  { id: "geometry", label: "骨格", ready: true },
  { id: "consonant", label: "子音", ready: false },
  { id: "scrub", label: "スクラブ", ready: false },
  { id: "export", label: "書き出し", ready: false },
];

const views = new Map<string, View>();
let current = "voice";
let pending = false;
function schedule() {
  if (pending) return;
  pending = true;
  requestAnimationFrame(() => { pending = false; views.get(current)?.render(); });
}

function setDrawer(open: boolean) {
  $("sidebar").classList.toggle("is-open", open);
  $("scrim").hidden = !open;
  $("menu").setAttribute("aria-expanded", String(open));
}

function show(id: string) {
  const v = views.get(id);
  if (!v) return;
  current = id;
  for (const [vid, view] of views) view.el.hidden = vid !== id;
  $("title").textContent = v.title;
  $("subtitle").textContent = v.subtitle;
  document.querySelectorAll<HTMLElement>(".nav-item[data-view]").forEach((b) => {
    if (b.dataset.view === id) b.setAttribute("aria-current", "page"); else b.removeAttribute("aria-current");
  });
  $("content").scrollTop = 0;
  if (location.hash.slice(1) !== id) history.replaceState(null, "", `#${id}`);
  schedule();
}

function buildNav() {
  const nav = $("nav");
  for (const n of NAV) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "nav-item";
    b.innerHTML = `<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${ICONS[n.id]}</svg><span>${n.label}</span>${n.ready ? "" : "<small>準備中</small>"}`;
    if (n.ready) {
      b.dataset.view = n.id;
      b.addEventListener("click", () => { show(n.id); setDrawer(false); });
    } else {
      b.setAttribute("aria-disabled", "true");
    }
    nav.appendChild(b);
  }
}

async function main() {
  await init();
  for (const v of [createVoiceView(schedule), createGeometryView(schedule)]) views.set(v.id, v);
  buildNav();

  $("menu").addEventListener("click", () => setDrawer(!$("sidebar").classList.contains("is-open")));
  $("scrim").addEventListener("click", () => setDrawer(false));
  window.addEventListener("keydown", (e) => { if (e.key === "Escape") setDrawer(false); });
  window.matchMedia("(min-width: 769px)").addEventListener("change", (e) => { if (e.matches) setDrawer(false); });
  $("reset").addEventListener("click", () => { views.get(current)?.reset(); schedule(); });
  window.addEventListener("hashchange", () => { const id = location.hash.slice(1); if (views.has(id) && id !== current) show(id); });
  new ResizeObserver(schedule).observe($("content"));

  const caps = detectCapabilities();
  $("env").textContent = `${caps.mode === "threaded" ? "高速モード" : "標準モード"} · core v${version()}`;

  const start = location.hash.slice(1);
  show(views.has(start) ? start : "voice");
}

main().catch((e) => {
  document.body.style.overflow = "auto";
  document.body.textContent = `起動に失敗しました: ${e}`;
});
