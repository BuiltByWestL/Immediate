// Save, Save As and Replace for photos.

import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import type { Edits } from "./edits";
import { mapper } from "./geometry";
import { paintMarkup } from "./markup";
import type { Pipeline } from "./pipeline";

export interface ImageInfo {
  iccName: string | null;
  colorSpace: "srgb" | "display-p3" | "other";
  saveFormat: string;
  saveExt: string;
  jpegQuality: number | null;
  lossless: boolean;
  /** Has an HDR gain map, which re-saving drops. */
  hdr: boolean;
}

/** Formats Advanced → Save offers. */
export const FORMATS = [
  { id: "jpeg", label: "JPEG", ext: "jpg" },
  { id: "png", label: "PNG", ext: "png" },
  { id: "webp", label: "WebP", ext: "webp" },
  { id: "tiff", label: "TIFF", ext: "tif" },
] as const;

/** Advanced → Save: what the saved photo looks like (null = as the original). */
export interface Output {
  format: string | null;
  quality: number | null;
  longSide: number | null;
  maxBytes: number | null;
  stripAll: boolean;
}

export const defaultOutput = (): Output => ({ format: null, quality: null, longSide: null, maxBytes: null, stripAll: false });

export type SaveTarget = "copy" | "replace" | "as";

/** Formats the Save As dialog offers, by extension. */
const BY_EXT: Record<string, string> = {
  jpg: "jpeg", jpeg: "jpeg", png: "png", webp: "webp", gif: "gif", bmp: "bmp",
  tif: "tiff", tiff: "tiff", tga: "tga", ico: "ico", ppm: "pnm", qoi: "qoi",
};

export interface SaveRequest {
  path: string;
  info: ImageInfo;
  target: SaveTarget;
  suffix: string;
  stripLocation: boolean;
  output: Output;
}

/** Renders the edits at full size and saves. Returns the saved path, or null if cancelled. */
export async function saveImage(pipeline: Pipeline, edits: Edits, r: SaveRequest): Promise<string | null> {
  let dest: string | null = null;
  // Advanced → Save's format applies to Save and Save As (Replace keeps the original's).
  let format: string | undefined = r.target !== "replace" ? (r.output.format ?? undefined) : undefined;
  const outExt = FORMATS.find((f) => f.id === format)?.ext ?? r.info.saveExt;
  if (r.target === "as") {
    const dot = r.path.lastIndexOf(".");
    const sep = Math.max(r.path.lastIndexOf("\\"), r.path.lastIndexOf("/"));
    const base = r.path.slice(0, dot > sep ? dot : undefined);
    dest = await saveDialog({
      defaultPath: `${base}${r.suffix}.${outExt}`,
      filters: [
        { name: (format ?? r.info.saveFormat).toUpperCase(), extensions: [outExt] },
        { name: "JPEG", extensions: ["jpg", "jpeg"] },
        { name: "PNG", extensions: ["png"] },
        { name: "WebP", extensions: ["webp"] },
        { name: "TIFF", extensions: ["tif", "tiff"] },
      ],
    });
    if (!dest) return null;
    let ext = dest.split(".").pop()?.toLowerCase() ?? "";
    if (!BY_EXT[ext] || dest.lastIndexOf(".") < Math.max(dest.lastIndexOf("\\"), dest.lastIndexOf("/"))) {
      dest = `${dest}.${outExt}`;
      ext = outExt;
    }
    format = BY_EXT[ext] ?? format ?? r.info.saveFormat;
  }

  // Let the "Saving…" state paint before the (blocking) full-size render.
  await new Promise((res) => requestAnimationFrame(() => setTimeout(res)));
  const { data, w, h } = renderFinal(pipeline, edits);
  const options = {
    source: r.path,
    width: w,
    height: h,
    target: r.target === "as" ? "path" : r.target,
    dest,
    suffix: r.suffix,
    format,
    quality: r.output.quality,
    stripLocation: r.stripLocation,
    stripMetadata: r.output.stripAll,
    resizeLong: r.output.longSide,
    targetBytes: r.output.maxBytes,
    // The pixels are in the original's color space unless it had an unusual profile, in which
    // case they were converted to sRGB and the profile must not be attached.
    keepIcc: r.info.colorSpace !== "other",
  };
  return invoke<string>("image_save", data, { headers: { "x-save": encodeURIComponent(JSON.stringify(options)) } });
}

/** The finished photo at full size: adjustments, geometry and markup. */
export function renderFinal(pipeline: Pipeline, edits: Edits): { data: Uint8Array; w: number; h: number } {
  const r = pipeline.renderFull(edits);
  return edits.markup.length ? { ...r, data: withMarkup(r.data, r.w, r.h, edits, pipeline) } : r;
}

/** Draws the markup onto the rendered pixels at full size. */
function withMarkup(data: Uint8Array, w: number, h: number, edits: Edits, pipeline: Pipeline): Uint8Array {
  const canvas = new OffscreenCanvas(w, h);
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  ctx.putImageData(new ImageData(new Uint8ClampedArray(data.buffer as ArrayBuffer, data.byteOffset, data.byteLength), w, h), 0, 0);
  const m = mapper(edits.geometry, pipeline.src);
  paintMarkup(ctx, edits.markup, (p) => m.toOutput(p[0], p[1]), 1);
  const out = ctx.getImageData(0, 0, w, h).data;
  return new Uint8Array(out.buffer, out.byteOffset, out.byteLength);
}
