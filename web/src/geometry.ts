// SPDX-License-Identifier: GPL-3.0-or-later
// Speaker geometry: data model, limits and JSON import/export.
// Limits mirror crates/core/src/geometry.rs.

export const SCHEMA = "vt2umora.speaker-geometry";
export const VERSION = 1;

export const LIMITS = {
  tractLengthCm: { min: 8, max: 22 },
  oralRatio: { min: 0.25, max: 0.75 },
  nasalVolumeScale: { min: 0.3, max: 2.5 },
} as const;

export interface Geometry {
  name: string;
  tractLengthCm: number;
  oralRatio: number;
  nasalVolumeScale: number;
}

export const PRESETS: { label: string; value: Geometry }[] = [
  { label: "成人男性", value: { name: "成人男性", tractLengthCm: 17.5, oralRatio: 0.5, nasalVolumeScale: 1 } },
  { label: "成人女性", value: { name: "成人女性", tractLengthCm: 14.8, oralRatio: 0.5, nasalVolumeScale: 1 } },
  { label: "子ども", value: { name: "子ども", tractLengthCm: 11.5, oralRatio: 0.5, nasalVolumeScale: 1 } },
];

export const defaultGeometry = (): Geometry => ({ ...PRESETS[0].value });

export function toJson(g: Geometry): string {
  return JSON.stringify(
    { schema: SCHEMA, version: VERSION, name: g.name, tractLengthCm: g.tractLengthCm, oralRatio: g.oralRatio, nasalVolumeScale: g.nasalVolumeScale },
    null, 2,
  ) + "\n";
}

export type ParseResult = { ok: true; value: Geometry } | { ok: false; error: string };

export function parseGeometry(text: string): ParseResult {
  let raw: unknown;
  try { raw = JSON.parse(text); } catch { return { ok: false, error: "JSON として読み込めません。" }; }
  if (typeof raw !== "object" || raw === null) return { ok: false, error: "JSON の形式が違います。" };
  const o = raw as Record<string, unknown>;
  if (o.schema !== SCHEMA) return { ok: false, error: "話者ジオメトリのファイルではありません。" };
  if (o.version !== VERSION) return { ok: false, error: `未対応のバージョンです (${String(o.version)})。` };

  const num = (key: keyof typeof LIMITS): number | string => {
    const v = o[key];
    const { min, max } = LIMITS[key];
    if (typeof v !== "number" || !Number.isFinite(v)) return `${key} が数値ではありません。`;
    if (v < min || v > max) return `${key} は ${min}〜${max} の範囲で指定してください。`;
    return v;
  };
  const len = num("tractLengthCm"), ratio = num("oralRatio"), nasal = num("nasalVolumeScale");
  for (const v of [len, ratio, nasal]) if (typeof v === "string") return { ok: false, error: v };
  const name = typeof o.name === "string" && o.name.trim() ? o.name.trim().slice(0, 40) : "無題";
  return { ok: true, value: { name, tractLengthCm: len as number, oralRatio: ratio as number, nasalVolumeScale: nasal as number } };
}

export function downloadJson(g: Geometry): void {
  const blob = new Blob([toJson(g)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  const safe = g.name.replace(/[^\p{L}\p{N}_-]+/gu, "_") || "speaker";
  a.download = `${safe}.speaker.json`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}
