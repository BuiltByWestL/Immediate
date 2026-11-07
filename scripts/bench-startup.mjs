// Startup budget check.
// Launches the release binary N times. Each run, the app writes its startup milestones
// (ms since process start) to a file and exits (see app_ready in src-tauri/src/lib.rs):
//   setup        Rust app set up, window + webview being created
//   pageStarted  webview began loading the page
//   navStart     webview navigation start (JS clock)
//   scriptStart  app bundle started executing
//   domContentLoaded
//   firstFrame   first frame painted
//   total        window shown
//
// Budgets (median of runs after the first):
//   windowShown ≤ 100 ms   native window on screen in the theme color
//   total       ≤ 400 ms   UI painted. On Windows ~270 ms of this is WebView2 starting its
//                          browser process, which the app cannot shorten.
//
// Usage: node scripts/bench-startup.mjs [runs=7] [contentBudgetMs=400] [windowBudgetMs=100]
// Run `npm run tauri build` first.

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const runs = Number(process.argv[2] ?? 7);
const budget = Number(process.argv[3] ?? 400);
const windowBudget = Number(process.argv[4] ?? 100);
const exe = resolve(
  "src-tauri/target/release",
  process.platform === "win32" ? "immediate.exe" : "immediate",
);
if (!existsSync(exe)) {
  console.error(`Not built: ${exe}\nRun: npm run tauri build`);
  process.exit(2);
}

const KEYS = ["setup", "windowShown", "webviewBuilt", "pageStarted", "navStart", "scriptStart", "domContentLoaded", "firstFrame", "pageFinished", "total"];
const dir = mkdtempSync(join(tmpdir(), "immediate-bench-"));
const results = [];

console.log(["run".padEnd(5), ...KEYS.map((k) => k.slice(0, 10).padStart(11))].join(""));
for (let i = 0; i < runs; i++) {
  const out = join(dir, `run-${i}.json`);
  const r = await new Promise((done, fail) => {
    const child = spawn(exe, [], { env: { ...process.env, IMMEDIATE_BENCH_OUT: out }, stdio: "ignore" });
    const timer = setTimeout(() => {
      child.kill();
      fail(new Error("app did not report ready within 10 s"));
    }, 10_000);
    child.on("exit", () => {
      clearTimeout(timer);
      existsSync(out) ? done(JSON.parse(readFileSync(out, "utf8"))) : fail(new Error("no measurement written"));
    });
  });
  results.push(r);
  console.log([String(i + 1).padEnd(5), ...KEYS.map((k) => String(r[k] ?? "-").padStart(11))].join(""));
  // Let the previous webview's helper processes exit so runs don't contend for its profile folder.
  await sleep(1500);
}
rmSync(dir, { recursive: true, force: true });

const medianOf = (key) => {
  const v = results.map((r) => r[key]);
  const rest = (v.length > 1 ? v.slice(1) : v).sort((a, b) => a - b);
  return rest[Math.floor(rest.length / 2)];
};
const win = medianOf("windowShown");
const content = medianOf("total");
console.log(`\nwindow visible ${win} ms (budget ${windowBudget}) · content ${content} ms (budget ${budget})`);
let ok = true;
if (!(win <= windowBudget)) (ok = false), console.error("FAIL: window shown too late");
if (!(content <= budget)) (ok = false), console.error("FAIL: content painted too late");
if (!ok) process.exit(1);
console.log("OK");
