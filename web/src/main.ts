import init, { version, lf_shape_from_rd } from "./wasm-pkg/vt2umora_wasm.js";
import { detectCapabilities } from "./capabilities";

async function main() {
  await init();
  const [ra, rk, rg] = lf_shape_from_rd(1.0);
  const caps = detectCapabilities();
  const app = document.getElementById("app")!;
  app.innerHTML = `
    <h1>VocalTract2UMora</h1>
    <ul>
      <li>wasm core: v${version()}</li>
      <li>Rd=1.0 → Ra=${ra.toFixed(3)}, Rk=${rk.toFixed(3)}, Rg=${rg.toFixed(3)}</li>
    </ul>
    <h2>Runtime mode: ${caps.mode}</h2>
    <pre>${JSON.stringify(caps, null, 2)}</pre>`;
}

main().catch((e) => {
  document.body.textContent = `Failed to start: ${e}`;
});
