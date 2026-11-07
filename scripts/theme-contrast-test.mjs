// Checks every theme (and every accent override on it) for legibility, using WCAG 2 contrast.
// "Faint" must never mean illegible. Fails with exit code 1 if any check fails.
//
// Usage: node scripts/theme-contrast-test.mjs [extra-theme-dir]
//   extra-theme-dir   also check custom themes (e.g. <config>/themes); failures there only warn

import { readdirSync, readFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";

const TOKENS = ["bg", "panel", "raised", "hover", "line", "lineStrong", "text", "textDim", "accent", "onAccent", "focus", "danger", "canvas"];

function rgb(hex) {
  const m = /^#([0-9a-f]{6})$/i.exec(hex);
  if (!m) throw new Error(`not a #rrggbb color: ${hex}`);
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
function luminance(hex) {
  const [r, g, b] = rgb(hex).map((c) => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function contrast(a, b) {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

// [foreground, background, minimum ratio, why]
const RULES = [
  ["text", "bg", 4.5, "body text"],
  ["text", "panel", 4.5, "text in panels"],
  ["text", "raised", 4.5, "text on buttons"],
  ["text", "hover", 4.5, "text on hovered rows"],
  ["textDim", "bg", 4.5, "secondary text"],
  ["textDim", "panel", 4.5, "secondary text in panels"],
  ["textDim", "raised", 3, "secondary text on buttons"],
  ["lineStrong", "bg", 3, "control boundaries"],
  ["lineStrong", "panel", 3, "control boundaries in panels"],
  ["accent", "bg", 3, "accent controls"],
  ["accent", "panel", 3, "accent controls in panels"],
  ["onAccent", "accent", 4.5, "text on accent buttons"],
  ["focus", "bg", 3, "focus ring"],
  ["focus", "panel", 3, "focus ring in panels"],
  ["danger", "bg", 4.5, "error text"],
  ["danger", "panel", 4.5, "error text in panels"],
];

function checkTheme(theme, accents = {}) {
  const errors = [];
  if (!["light", "dark"].includes(theme.scheme)) errors.push(`scheme must be "light" or "dark"`);
  const t = theme.tokens ?? {};
  for (const k of TOKENS) if (!t[k]) errors.push(`missing token ${k}`);
  if (errors.length) return errors;
  const check = (tokens, label, only) => {
    for (const [fg, bg, min, why] of RULES) {
      if (only && !only.includes(fg)) continue;
      const r = contrast(tokens[fg], tokens[bg]);
      if (r < min) errors.push(`${label}${fg} on ${bg} = ${r.toFixed(2)}:1, needs ${min}:1 (${why})`);
    }
  };
  check(t, "");
  // The canvas behind photos and video must stay neutral so a theme never skews color judgement.
  const [r, g, b] = rgb(t.canvas);
  if (r !== g || g !== b) errors.push(`canvas ${t.canvas} must be a neutral grey (r = g = b)`);
  for (const [id, a] of Object.entries(accents)) {
    if (id.startsWith("$")) continue;
    const v = a[theme.scheme];
    check({ ...t, accent: v.accent, onAccent: v.onAccent, focus: v.accent }, `accent ${id}: `, ["accent", "onAccent", "focus"]);
  }
  return errors;
}

function loadDir(dir) {
  return readdirSync(dir)
    .filter((f) => f.endsWith(".json"))
    .map((f) => ({ file: f, theme: JSON.parse(readFileSync(join(dir, f), "utf8")) }));
}

{
  const root = resolve(import.meta.dirname, "../src/design");
  const accents = JSON.parse(readFileSync(join(root, "accents.json"), "utf8"));
  let failed = 0;
  for (const { file, theme } of loadDir(join(root, "themes"))) {
    const errors = checkTheme(theme, accents);
    console.log(`${errors.length ? "FAIL" : "ok  "}  ${theme.name ?? file}`);
    for (const e of errors) console.log(`        ${e}`);
    if (errors.length) failed++;
  }
  const extra = process.argv[2];
  if (extra && existsSync(extra)) {
    for (const { file, theme } of loadDir(extra)) {
      const errors = checkTheme(theme);
      console.log(`${errors.length ? "warn" : "ok  "}  ${theme.name ?? file} (custom)`);
      for (const e of errors) console.log(`        ${e}`);
    }
  }
  if (failed) {
    console.error(`\n${failed} theme(s) failed the contrast check.`);
    process.exit(1);
  }
  console.log("\nAll themes pass.");
}
