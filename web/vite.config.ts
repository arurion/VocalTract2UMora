import { defineConfig } from "vite";

// GitHub Pages serves under /<repo>/, so CI passes VITE_BASE.
export default defineConfig({
  base: process.env.VITE_BASE ?? "/",
  worker: { format: "es" },
  build: { target: "es2022" },
  // Localhost: enable cross-origin isolation natively (SharedArrayBuffer).
  // On GitHub Pages coi-serviceworker provides the same headers.
  server: {
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
  },
  preview: {
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
  },
});
