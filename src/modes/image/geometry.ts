// Geometry math shared by the crop tool, the renderer and export.
// Spaces: "source" = the upright photo; "rotated" = after quarter turns (and flips), where the
// crop rectangle lives as fractions 0..1; straighten and perspective act around its center.

import type { Geometry } from "./edits";

export interface Size {
  w: number;
  h: number;
}
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function rotatedSize(src: Size, g: Geometry): Size {
  return g.quarter % 2 ? { w: src.h, h: src.w } : src;
}

/** Maps a point in rotated space (fractions) to source UV (fractions); mirrors the shader. */
export function sourceUV(rx: number, ry: number, g: Geometry, src: Size): [number, number] {
  const rot = rotatedSize(src, g);
  let px = (rx - 0.5) * rot.w;
  let py = (ry - 0.5) * rot.h;
  const a = (-g.angle * Math.PI) / 180;
  const s = Math.sin(a);
  const c = Math.cos(a);
  // GLSL mat2(c, s, -s, c) * p (column-major)
  [px, py] = [c * px - s * py, s * px + c * py];
  let qx = px / rot.w;
  let qy = py / rot.h;
  [qx, qy] = [qx * (1 + (g.persV / 100) * qy * 1.2), qy * (1 + (g.persH / 100) * qx * 1.2)];
  px = qx * rot.w * (g.flipH ? -1 : 1);
  py = qy * rot.h * (g.flipV ? -1 : 1);
  for (let i = 0; i < g.quarter % 4; i++) [px, py] = [py, -px];
  return [px / src.w + 0.5, py / src.h + 0.5];
}

/** Whether every point of the crop shows part of the photo (no empty corners). */
export function cropInside(crop: Rect, g: Geometry, src: Size): boolean {
  const eps = 1e-4;
  for (let i = 0; i <= 4; i++) {
    for (const [fx, fy] of [[i / 4, 0], [i / 4, 1], [0, i / 4], [1, i / 4]]) {
      const [u, v] = sourceUV(crop.x + fx * crop.w, crop.y + fy * crop.h, g, src);
      if (u < -eps || v < -eps || u > 1 + eps || v > 1 + eps) return false;
    }
  }
  return true;
}

/**
 * The crop actually used: the user's crop, shrunk around its center just enough to stay inside
 * the photo after straightening/perspective. Keeping the user's crop separate means
 * straightening back to 0 restores it.
 */
export function effectiveCrop(g: Geometry, src: Size): Rect {
  const c = g.crop;
  if (cropInside(c, g, src)) return c;
  const cx = c.x + c.w / 2;
  const cy = c.y + c.h / 2;
  const scaled = (k: number, x = cx, y = cy): Rect => ({ x: x - (c.w * k) / 2, y: y - (c.h * k) / 2, w: c.w * k, h: c.h * k });
  // If even a tiny crop at this center falls outside, shrink toward the middle instead.
  const [x, y] = cropInside(scaled(0.02), g, src) ? [cx, cy] : [0.5, 0.5];
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 24; i++) {
    const mid = (lo + hi) / 2;
    if (cropInside(scaled(mid, x, y), g, src)) lo = mid;
    else hi = mid;
  }
  return scaled(lo, x, y);
}

export function outputSize(src: Size, g: Geometry): Size {
  const rot = rotatedSize(src, g);
  const c = effectiveCrop(g, src);
  return { w: Math.max(1, Math.round(rot.w * c.w)), h: Math.max(1, Math.round(rot.h * c.h)) };
}

/**
 * The area the crop tool shows, in rotated-space fractions: the whole straightened photo, so its
 * corners aren't clipped by the original frame. (Perspective adds a little margin.)
 */
export function cropViewBox(g: Geometry, src: Size): Rect {
  const rot = rotatedSize(src, g);
  const a = (Math.abs(g.angle) * Math.PI) / 180;
  const bw = rot.w * Math.cos(a) + rot.h * Math.sin(a);
  const bh = rot.w * Math.sin(a) + rot.h * Math.cos(a);
  const m = 1 + (0.35 * Math.max(Math.abs(g.persV), Math.abs(g.persH))) / 100;
  const w = (bw / rot.w) * m;
  const h = (bh / rot.h) * m;
  return { x: (1 - w) / 2, y: (1 - h) / 2, w, h };
}

/** Largest rect with aspect `ratio` (w/h, in pixels) centered in `within`. */
export function fitAspect(within: Rect, ratio: number, rot: Size): Rect {
  const pw = within.w * rot.w;
  const ph = within.h * rot.h;
  let w = pw;
  let h = pw / ratio;
  if (h > ph) {
    h = ph;
    w = ph * ratio;
  }
  const fw = w / rot.w;
  const fh = h / rot.h;
  return { x: within.x + (within.w - fw) / 2, y: within.y + (within.h - fh) / 2, w: fw, h: fh };
}

/** Geometry after a quarter turn to the left (counter-clockwise), keeping the crop on the same content. */
export function rotateLeft(g: Geometry): Geometry {
  const c = g.crop;
  return {
    ...g,
    quarter: (g.quarter + 3) % 4,
    // A mirror followed by a quarter turn is the same as the turn followed by the other mirror.
    flipH: g.flipV,
    flipV: g.flipH,
    crop: { x: c.y, y: 1 - (c.x + c.w), w: c.h, h: c.w },
    aspect: g.aspect ? 1 / g.aspect : null,
    persH: g.persV,
    persV: g.persH,
  };
}

/** Mirror left-right, keeping the crop on the same content and straighten looking the same. */
export function flipHorizontal(g: Geometry): Geometry {
  const c = g.crop;
  return { ...g, flipH: !g.flipH, crop: { ...c, x: 1 - (c.x + c.w) }, angle: -g.angle, persH: -g.persH };
}
