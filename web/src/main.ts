import init, { version, lf_shape_from_rd } from "./wasm-pkg/vt2umora_wasm.js";

async function main() {
  await init();
  const [ra, rk, rg] = lf_shape_from_rd(1.0);
  const app = document.getElementById("app")!;
  app.innerHTML = `
    <h1>VocalTract2UMora</h1>
    <ul>
      <li>wasm core: v${version()}</li>
      <li>Rd=1.0 → Ra=${ra.toFixed(3)}, Rk=${rk.toFixed(3)}, Rg=${rg.toFixed(3)}</li>
      <li>crossOriginIsolated: ${self.crossOriginIsolated}</li>
      <li>SharedArrayBuffer: ${typeof SharedArrayBuffer !== "undefined"}</li>
    </ul>`;
}

main().catch((e) => {
  document.body.textContent = `Failed to start: ${e}`;
});
