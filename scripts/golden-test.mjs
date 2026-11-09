// Golden-image test for the photo pipeline: serves tests/golden/ with Vite, opens it in a
// headless Chrome or Edge already on the machine (playwright-core, no browser download) with
// software WebGL (SwiftShader, the same on every machine), and checks the results.
//
//   node scripts/golden-test.mjs            check
//   node scripts/golden-test.mjs --update   write new reference pictures (after an intended change)

import { writeFileSync } from "node:fs";
import { chromium } from "playwright-core";
import { createServer } from "vite";

const update = process.argv.includes("--update");
// Preview vs saved: both are the same shader; only the scaling path differs.
const PREVIEW = { mean: 2.5, p99: 16 };
// Against the reference: small differences between SwiftShader versions are fine.
const REFERENCE = { mean: 1.5, p99: 10 };

// The app's Vite config (Svelte plugin), on any free port.
const server = await createServer({ server: { port: 0, strictPort: false, host: "127.0.0.1", hmr: false }, logLevel: "error", optimizeDeps: { entries: ["tests/golden/index.html"] } });
await server.listen();
const { port } = server.httpServer.address();

let browser;
for (const channel of ["chrome", "msedge", "chromium"]) {
  try {
    browser = await chromium.launch({ channel, headless: true, args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
    break;
  } catch {
    /* try the next browser */
  }
}
if (!browser) {
  await server.close();
  console.error("No Chrome or Edge found for the golden-image test.");
  process.exit(1);
}

let failed = false;
try {
  const page = await browser.newPage();
  page.on("pageerror", (e) => console.error("page error:", e.message));
  await page.goto(`http://127.0.0.1:${port}/tests/golden/index.html`);
  const results = await page.evaluate(() => window.results);
  if (results.error) throw new Error(results.error);
  for (const [name, r] of Object.entries(results)) {
    const fmt = (d) => (d ? `mean ${d.mean.toFixed(2)}, p99 ${d.p99}` : "no reference");
    const previewOk = r.previewVsSaved.mean <= PREVIEW.mean && r.previewVsSaved.p99 <= PREVIEW.p99;
    const refOk = update || (r.vsReference && r.vsReference.mean <= REFERENCE.mean && r.vsReference.p99 <= REFERENCE.p99);
    if (!previewOk || !refOk) failed = true;
    console.log(`${previewOk && refOk ? "ok  " : "FAIL"} ${name.padEnd(9)} ${r.size.join("x").padEnd(8)} preview vs saved: ${fmt(r.previewVsSaved)} | vs reference: ${fmt(r.vsReference)}`);
    if (update) writeFileSync(`tests/golden/${name}.png`, Buffer.from(r.png.split(",")[1], "base64"));
  }
  if (update) console.log("References written to tests/golden/.");
} finally {
  await browser.close();
  await server.close();
}
process.exit(failed ? 1 : 0);
