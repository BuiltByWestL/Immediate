// Golden-image test, run in a headless browser by scripts/golden-test.mjs. Renders a fixed
// synthetic photo through the real photo pipeline (src/modes/image/pipeline.ts) with a set of
// edits and checks:
//   1. what you see is what you save: the on-screen preview at half size matches the full-size
//      saved render scaled down the same way;
//   2. nothing drifted: each saved render matches its reference picture in tests/golden/.
// The page reports through window.results; the runner writes new references when asked.

import { neutralEdits, type Edits } from "../../src/modes/image/edits";
import { Pipeline } from "../../src/modes/image/pipeline";

const W = 600;
const H = 400;
/** References are stored this wide, in the render's shape (small files; enough to catch any
 *  visible change). */
const REF_W = 300;

/** A deterministic test photo: gradients, saturated patches, fine detail, skin and sky tones. */
function testPhoto(): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = W;
  c.height = H;
  const g = c.getContext("2d")!;
  const sky = g.createLinearGradient(0, 0, 0, H);
  sky.addColorStop(0, "#3b6fb6");
  sky.addColorStop(0.55, "#a9c8e8");
  sky.addColorStop(0.56, "#6b8f3a");
  sky.addColorStop(1, "#2f4a1c");
  g.fillStyle = sky;
  g.fillRect(0, 0, W, H);
  const swatches = ["#e03a2f", "#f2a33a", "#f5dd3b", "#46b05a", "#2f7fe0", "#8a4fd6", "#d9a38a", "#ffffff", "#808080", "#101010"];
  swatches.forEach((col, i) => {
    g.fillStyle = col;
    g.fillRect(20 + i * 56, 250, 48, 48);
  });
  // Fine detail for sharpening and noise reduction.
  for (let x = 20; x < 300; x += 4) {
    g.fillStyle = x % 8 ? "#222" : "#ddd";
    g.fillRect(x, 320, 2, 60);
  }
  const ramp = g.createLinearGradient(320, 0, 580, 0);
  ramp.addColorStop(0, "#000");
  ramp.addColorStop(1, "#fff");
  g.fillStyle = ramp;
  g.fillRect(320, 320, 260, 60);
  g.fillStyle = "#f6e7a8";
  g.beginPath();
  g.arc(470, 90, 45, 0, Math.PI * 2);
  g.fill();
  return c;
}

function edits(patch: (e: Edits) => void): Edits {
  const e = neutralEdits();
  patch(e);
  return e;
}

const CASES: Record<string, Edits> = {
  neutral: edits(() => {}),
  light: edits((e) => Object.assign(e.adjust, { exposure: 35, highlights: -40, shadows: 45, contrast: 25, blackPoint: 15, brilliance: 30 })),
  color: edits((e) => Object.assign(e.adjust, { saturation: 40, vibrance: 30, warmth: 35, tint: -20 })),
  detail: edits((e) => Object.assign(e.adjust, { sharpness: 60, definition: 50, noise: 40, vignette: 45 })),
  bw: edits((e) => Object.assign(e.adjust, { bw: 100, bwNeutrals: 30, bwTone: -25 })),
  dramatic: edits((e) => (e.filter = { id: "dramatic-warm", amount: 80 })),
  geometry: edits((e) => {
    e.geometry.quarter = 1;
    e.geometry.flipH = true;
    e.geometry.angle = 6;
    e.geometry.crop = { x: 0.1, y: 0.08, w: 0.75, h: 0.8 };
  }),
};

/** Pixels of a canvas-like source at w × h (smooth scaling). */
function pixels(src: CanvasImageSource, w: number, h: number): Uint8ClampedArray {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const g = c.getContext("2d", { willReadFrequently: true })!;
  g.imageSmoothingQuality = "high";
  g.drawImage(src, 0, 0, w, h);
  return g.getImageData(0, 0, w, h).data;
}

function toCanvas(data: Uint8Array | Uint8ClampedArray, w: number, h: number): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  c.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(data), w, h), 0, 0);
  return c;
}

/** Mean and 99th-percentile absolute difference per color channel (0–255), alpha ignored. */
function diff(a: Uint8ClampedArray, b: Uint8ClampedArray) {
  const d: number[] = [];
  for (let i = 0; i < a.length; i += 4) for (let k = 0; k < 3; k++) d.push(Math.abs(a[i + k] - b[i + k]));
  const sorted = d.slice().sort((x, y) => x - y);
  return { mean: d.reduce((s, v) => s + v, 0) / d.length, p99: sorted[Math.floor(sorted.length * 0.99)] };
}

async function reference(name: string): Promise<HTMLImageElement | null> {
  const res = await fetch(`/tests/golden/${name}.png`, { cache: "no-store" });
  // A missing file comes back as the app's page, not a 404.
  if (!res.ok || !res.headers.get("content-type")?.startsWith("image/")) return null;
  const img = new Image();
  img.src = URL.createObjectURL(await res.blob());
  await img.decode();
  return img;
}

async function run() {
  const canvas = document.createElement("canvas");
  const p = new Pipeline(canvas);
  p.setSource(testPhoto(), { w: W, h: H });
  const out: Record<string, unknown> = {};
  for (const [name, e] of Object.entries(CASES)) {
    const full = p.renderFull(e);
    const fullCanvas = toCanvas(full.data, full.w, full.h);
    // The preview: the same edits drawn on screen at half size.
    const pw = Math.round(full.w / 2);
    const ph = Math.round(full.h / 2);
    canvas.width = pw;
    canvas.height = ph;
    p.render(e, { scale: 0.5, x: 0, y: 0 });
    const preview = pixels(canvas, pw, ph);
    const saved = pixels(fullCanvas, pw, ph);
    const seen = diff(preview, saved);
    const refImg = await reference(name);
    const refH = Math.round((REF_W * full.h) / full.w);
    const mine = pixels(fullCanvas, REF_W, refH);
    const golden = refImg ? diff(mine, pixels(refImg, REF_W, refH)) : null;
    out[name] = { size: [full.w, full.h], previewVsSaved: seen, vsReference: golden, png: toCanvas(mine, REF_W, refH).toDataURL("image/png") };
  }
  p.dispose();
  return out;
}

(window as any).results = run().catch((e) => ({ error: String(e) }));
