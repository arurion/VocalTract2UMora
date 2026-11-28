// SPDX-License-Identifier: GPL-3.0-or-later
// Messages between the batch runner (main thread) and its workers.

export type Style = "single" | "vcv" | "cvvc";

export interface RenderSettings {
  style: Style;
  f0Hz: number;
  vowelHoldS: number;
  /** 1 = exact, 2 = default, 4 = fast (see the Rust renderer). */
  geometryStride: number;
  sampleRate: number;
  seed: number;
}

export type ToWorker =
  | { type: "init"; speaker: string }
  | { type: "job"; id: number; index: number; text: string; settings: RenderSettings };

export type FromWorker =
  | { type: "ready" }
  | { type: "done"; id: number; name: string; wav: Uint8Array; aliases: string[]; times: number[][] }
  | { type: "fail"; id: number; message: string };
