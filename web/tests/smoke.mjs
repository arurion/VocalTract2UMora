// SPDX-License-Identifier: GPL-3.0-or-later
// Browser smoke test of the production build (CI): the app must start (no exception before the
// first view), every view must open, and a two-line voicebank must be generated and downloaded.
//
//   npm run build && npm install --no-save playwright@1.48.0 && npx playwright install chromium
//   node tests/smoke.mjs        (VITE_BASE must be the same as at build time)
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { chromium } from "playwright";

const base = process.env.VITE_BASE ?? "/";
const port = 4173;
const origin = `http://localhost:${port}`;
const fail = (msg) => { console.error(`SMOKE FAIL: ${msg}`); process.exitCode = 1; };

// Own process group, so that the whole `npx -> vite` tree can be stopped at the end (a leftover
// child would keep the pipe open and the script would never exit).
const server = spawn("npx", ["vite", "preview", "--port", String(port), "--strictPort"], { stdio: ["ignore", "pipe", "inherit"], env: process.env, detached: true });
const stopServer = () => { try { process.kill(-server.pid, "SIGKILL"); } catch { /* already gone */ } };
try {
  await new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error("vite preview did not start")), 30000);
    server.stdout.on("data", (d) => { if (String(d).includes("localhost")) { clearTimeout(t); resolve(); } });
    server.on("exit", (c) => reject(new Error(`vite preview exited (${c})`)));
  });
} catch (e) {
  stopServer();
  console.error(`SMOKE FAIL: ${e}`);
  process.exit(1);
}

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ acceptDownloads: true });
  const errors = [];
  page.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
  page.on("console", (m) => { if (m.type() === "error") errors.push(`console: ${m.text()}`); });

  await page.goto(`${origin}${base}#export`);
  // coi-serviceworker may reload the page once; wait for the app itself.
  await page.waitForFunction(() => document.getElementById("title")?.textContent === "音源の書き出し", null, { timeout: 60000 });
  const info = await page.textContent("#e-list-info");
  if (!/145 行/.test(info ?? "")) fail(`built-in list not read: "${info}"`);

  for (const [view, title] of [["voice", "声質エディタ"], ["geometry", "骨格エディタ"], ["export", "音源の書き出し"]]) {
    await page.click(`.nav-item[data-view="${view}"]`);
    const got = await page.textContent("#title");
    if (got !== title) fail(`view ${view}: title "${got}", expected "${title}"`);
  }

  await page.evaluate(() => {
    const ta = document.getElementById("e-list");
    ta.value = "_あか\n_かあ";
    ta.dispatchEvent(new Event("input"));
  });
  await page.click(".btn.primary");
  await page.waitForFunction(() => document.getElementById("e-p-title")?.textContent === "完了", null, { timeout: 180000 });
  const status = await page.textContent("#e-controls .status[role=status]");
  if (!/oto \d+ 件 · WAV 2 本/.test(status ?? "")) fail(`unexpected result: "${status}"`);

  const [download] = await Promise.all([
    page.waitForEvent("download"),
    page.click("text=ZIP をダウンロード"),
  ]);
  const bytes = await readFile(await download.path());
  if (bytes.subarray(0, 2).toString() !== "PK" || !bytes.includes(Buffer.from("oto.ini"))) fail("the downloaded file is not a voicebank ZIP");

  if (errors.length) fail(`errors in the page:\n  ${errors.join("\n  ")}`);
  if (!process.exitCode) console.log("smoke test passed:", status);
} catch (e) {
  fail(String(e));
} finally {
  await browser.close();
  stopServer();
}
process.exit(process.exitCode ?? 0);
