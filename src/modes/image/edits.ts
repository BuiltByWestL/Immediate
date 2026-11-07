// The edit model for photos: plain data, so undo is a stack of snapshots and nothing is ever
// written until Save. All slider values run -100..100 (0 = untouched) unless noted.

export interface Adjust {
  exposure: number;
  brilliance: number;
  highlights: number;
  shadows: number;
  contrast: number;
  brightness: number;
  blackPoint: number;
  saturation: number;
  vibrance: number;
  warmth: number;
  tint: number;
  sharpness: number; // 0..100
  definition: number; // 0..100
  noise: number; // 0..100 (noise reduction)
  vignette: number; // -100..100 (negative lightens the corners)
  // Black & white
  bw: number; // 0..100 intensity
  bwNeutrals: number;
  bwTone: number;
  grain: number; // 0..100
}

export interface Geometry {
  /** Quarter turns clockwise (0..3). */
  quarter: number;
  flipH: boolean;
  flipV: boolean;
  /** Straighten, degrees (-45..45). */
  angle: number;
  /** Perspective, -100..100. */
  persV: number;
  persH: number;
  /** Crop rectangle in the rotated image, as fractions (0..1). */
  crop: { x: number; y: number; w: number; h: number };
  /** Locked aspect ratio (w/h) or null for free. */
  aspect: number | null;
}

export interface Edits {
  adjust: Adjust;
  filter: { id: string | null; amount: number }; // amount 0..100
  geometry: Geometry;
  /** Auto-enhance: slider offsets computed from the photo, applied on top (null = off). */
  auto: Partial<Adjust> | null;
}

export const NEUTRAL_ADJUST: Adjust = {
  exposure: 0, brilliance: 0, highlights: 0, shadows: 0, contrast: 0, brightness: 0, blackPoint: 0,
  saturation: 0, vibrance: 0, warmth: 0, tint: 0,
  sharpness: 0, definition: 0, noise: 0, vignette: 0,
  bw: 0, bwNeutrals: 0, bwTone: 0, grain: 0,
};

export const NEUTRAL_GEOMETRY: Geometry = {
  quarter: 0, flipH: false, flipV: false, angle: 0, persV: 0, persH: 0,
  crop: { x: 0, y: 0, w: 1, h: 1 }, aspect: null,
};

export function neutralEdits(): Edits {
  return {
    adjust: { ...NEUTRAL_ADJUST },
    filter: { id: null, amount: 100 },
    geometry: structuredClone(NEUTRAL_GEOMETRY),
    auto: null,
  };
}

/** Deep copy (works on Svelte state proxies too). */
export const clone = (e: Edits): Edits => JSON.parse(JSON.stringify(e));

export function isNeutral(e: Edits): boolean {
  return JSON.stringify(e.adjust) === JSON.stringify(NEUTRAL_ADJUST) && !e.filter.id && !e.auto && !geometryChanged(e.geometry);
}

export function geometryChanged(g: Geometry): boolean {
  const c = g.crop;
  return g.quarter !== 0 || g.flipH || g.flipV || g.angle !== 0 || g.persV !== 0 || g.persH !== 0 ||
    c.x !== 0 || c.y !== 0 || c.w !== 1 || c.h !== 1;
}

// ---- sliders -------------------------------------------------------------------------------

export type AdjustKey = keyof Adjust;
export interface SliderDef {
  key: AdjustKey;
  label: string;
  min: number;
  max: number;
  /** Shown in Simple mode. */
  simple?: boolean;
}

export const SECTIONS: { title: string; sliders: SliderDef[] }[] = [
  {
    title: "Light",
    sliders: [
      { key: "exposure", label: "Exposure", min: -100, max: 100, simple: true },
      { key: "brilliance", label: "Brilliance", min: -100, max: 100 },
      { key: "highlights", label: "Highlights", min: -100, max: 100 },
      { key: "shadows", label: "Shadows", min: -100, max: 100 },
      { key: "contrast", label: "Contrast", min: -100, max: 100, simple: true },
      { key: "brightness", label: "Brightness", min: -100, max: 100, simple: true },
      { key: "blackPoint", label: "Black point", min: -100, max: 100 },
    ],
  },
  {
    title: "Color",
    sliders: [
      { key: "saturation", label: "Saturation", min: -100, max: 100, simple: true },
      { key: "vibrance", label: "Vibrance", min: -100, max: 100 },
      { key: "warmth", label: "Warmth", min: -100, max: 100, simple: true },
      { key: "tint", label: "Tint", min: -100, max: 100 },
    ],
  },
  {
    title: "Black & white",
    sliders: [
      { key: "bw", label: "Intensity", min: 0, max: 100 },
      { key: "bwNeutrals", label: "Neutrals", min: -100, max: 100 },
      { key: "bwTone", label: "Tone", min: -100, max: 100 },
      { key: "grain", label: "Grain", min: 0, max: 100 },
    ],
  },
  {
    title: "Detail",
    sliders: [
      { key: "sharpness", label: "Sharpness", min: 0, max: 100 },
      { key: "definition", label: "Definition", min: 0, max: 100 },
      { key: "noise", label: "Noise reduction", min: 0, max: 100 },
    ],
  },
  {
    title: "Effects",
    sliders: [{ key: "vignette", label: "Vignette", min: -100, max: 100 }],
  },
];

/** Advanced-only edits that are active, so Simple mode can say they're still applied. */
export function hiddenEditCount(e: Edits): number {
  let n = 0;
  for (const s of SECTIONS) for (const d of s.sliders) if (!d.simple && e.adjust[d.key] !== 0) n++;
  const g = e.geometry;
  if (g.persH || g.persV) n++;
  return n;
}

// ---- filters -------------------------------------------------------------------------------

/** A filter is a set of slider offsets, scaled by its amount. */
export interface Filter {
  id: string;
  name: string;
  offsets: Partial<Adjust>;
}

export const FILTERS: Filter[] = [
  { id: "vivid", name: "Vivid", offsets: { saturation: 22, vibrance: 20, contrast: 14, brilliance: 8 } },
  { id: "vivid-warm", name: "Vivid Warm", offsets: { saturation: 18, vibrance: 18, contrast: 14, warmth: 28, brilliance: 8 } },
  { id: "vivid-cool", name: "Vivid Cool", offsets: { saturation: 18, vibrance: 18, contrast: 14, warmth: -28, brilliance: 8 } },
  { id: "dramatic", name: "Dramatic", offsets: { contrast: 30, definition: 40, highlights: -30, shadows: -18, saturation: -18, vignette: 22 } },
  { id: "dramatic-warm", name: "Dramatic Warm", offsets: { contrast: 30, definition: 40, highlights: -30, shadows: -18, saturation: -10, warmth: 30, vignette: 22 } },
  { id: "dramatic-cool", name: "Dramatic Cool", offsets: { contrast: 30, definition: 40, highlights: -30, shadows: -18, saturation: -10, warmth: -30, vignette: 22 } },
  { id: "mono", name: "Mono", offsets: { bw: 100 } },
  { id: "silvertone", name: "Silvertone", offsets: { bw: 100, contrast: 26, bwTone: 30, brilliance: 10 } },
  { id: "noir", name: "Noir", offsets: { bw: 100, contrast: 55, blackPoint: 30, definition: 25, vignette: 30 } },
];

const RANGES = new Map(SECTIONS.flatMap((s) => s.sliders.map((d) => [d.key, d] as const)));

/** What the renderer uses: sliders + Auto + the filter's offsets, clamped. */
export function effectiveAdjust(e: Edits): Adjust {
  const out = { ...e.adjust };
  const add = (offsets: Partial<Adjust>, k: number) => {
    for (const [key, v] of Object.entries(offsets) as [AdjustKey, number][]) {
      const r = RANGES.get(key)!;
      out[key] = Math.max(r.min, Math.min(r.max, out[key] + v * k));
    }
  };
  if (e.auto) add(e.auto, 1);
  const f = FILTERS.find((x) => x.id === e.filter.id);
  if (f) add(f.offsets, e.filter.amount / 100);
  return out;
}

/**
 * Auto-enhance: reads a small luminance histogram of the photo and returns slider offsets that
 * bring a dull, dark or flat photo to a balanced exposure with a little more life.
 */
export function autoEnhance(rgba: Uint8Array): Partial<Adjust> {
  const hist = new Uint32Array(256);
  let n = 0;
  let satSum = 0;
  for (let i = 0; i < rgba.length; i += 4) {
    const r = rgba[i], g = rgba[i + 1], b = rgba[i + 2];
    hist[Math.round(0.2126 * r + 0.7152 * g + 0.0722 * b)]++;
    satSum += (Math.max(r, g, b) - Math.min(r, g, b)) / 255;
    n++;
  }
  const pct = (p: number) => {
    let acc = 0;
    for (let i = 0; i < 256; i++) if ((acc += hist[i]) >= n * p) return i / 255;
    return 1;
  };
  const lo = pct(0.005), p10 = pct(0.1), mid = pct(0.5), p90 = pct(0.9), hi = pct(0.995);
  const sat = satSum / Math.max(1, n);
  const clamp = (v: number, a: number, b: number) => Math.round(Math.max(a, Math.min(b, v)));
  const out: Partial<Adjust> = {
    // Aim the middle tone at ~0.46; exposure 100 = +2 EV.
    exposure: clamp((Math.log2(0.46 / Math.max(0.02, mid)) / 2) * 100 * 0.6, -45, 45),
    blackPoint: clamp(lo > 0.03 ? lo * 250 : 0, 0, 25),
    highlights: clamp(hi > 0.96 ? -20 - (hi - 0.96) * 400 : 0, -40, 0),
    shadows: clamp(p10 < 0.08 ? 18 : p10 < 0.15 ? 10 : 0, 0, 25),
    contrast: clamp((0.55 - (p90 - p10)) * 40, -10, 15),
    vibrance: clamp((0.32 - sat) * 80, 0, 25),
  };
  for (const k of Object.keys(out) as AdjustKey[]) if (!out[k]) delete out[k];
  return out;
}
