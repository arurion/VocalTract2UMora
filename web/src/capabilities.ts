// SPDX-License-Identifier: GPL-3.0-or-later
// Runtime capability detection. Everything here is OPTIONAL acceleration:
// the app must work with all flags false (e.g. Android WebView browsers such as Via,
// where crossOriginIsolated stays false even with COOP/COEP headers).

export interface Capabilities {
  secureContext: boolean;
  crossOriginIsolated: boolean;
  sharedArrayBuffer: boolean;
  worker: boolean;
  audioWorklet: boolean;
  serviceWorkerControlled: boolean;
  /** "threaded": shared memory available. "basic": single-thread Wasm + postMessage. */
  mode: "threaded" | "basic";
}

export function detectCapabilities(): Capabilities {
  const isolated = self.crossOriginIsolated === true;
  const sab = typeof SharedArrayBuffer !== "undefined";
  return {
    secureContext: self.isSecureContext,
    crossOriginIsolated: isolated,
    sharedArrayBuffer: sab,
    worker: typeof Worker !== "undefined",
    audioWorklet:
      typeof AudioContext !== "undefined" && "audioWorklet" in AudioContext.prototype,
    serviceWorkerControlled: !!navigator.serviceWorker?.controller,
    mode: isolated && sab ? "threaded" : "basic",
  };
}
