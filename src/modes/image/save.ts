// Save, Save As and Replace for photos.

import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import type { Edits } from "./edits";
import type { Pipeline } from "./pipeline";

export interface ImageInfo {
  iccName: string | null;
  colorSpace: "srgb" | "display-p3" | "other";
  saveFormat: string;
  saveExt: string;
  jpegQuality: number | null;
  lossless: boolean;
}

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
  stripMetadata: boolean;
}

/** Renders the edits at full size and saves. Returns the saved path, or null if cancelled. */
export async function saveImage(pipeline: Pipeline, edits: Edits, r: SaveRequest): Promise<string | null> {
  let dest: string | null = null;
  let format: string | undefined;
  if (r.target === "as") {
    const dot = r.path.lastIndexOf(".");
    const sep = Math.max(r.path.lastIndexOf("\\"), r.path.lastIndexOf("/"));
    const base = r.path.slice(0, dot > sep ? dot : undefined);
    dest = await saveDialog({
      defaultPath: `${base}${r.suffix}.${r.info.saveExt}`,
      filters: [
        { name: r.info.saveFormat.toUpperCase(), extensions: [r.info.saveExt] },
        { name: "JPEG", extensions: ["jpg", "jpeg"] },
        { name: "PNG", extensions: ["png"] },
        { name: "WebP", extensions: ["webp"] },
        { name: "TIFF", extensions: ["tif", "tiff"] },
      ],
    });
    if (!dest) return null;
    let ext = dest.split(".").pop()?.toLowerCase() ?? "";
    if (!BY_EXT[ext] || dest.lastIndexOf(".") < Math.max(dest.lastIndexOf("\\"), dest.lastIndexOf("/"))) {
      dest = `${dest}.${r.info.saveExt}`;
      ext = r.info.saveExt;
    }
    format = BY_EXT[ext] ?? r.info.saveFormat;
  }

  // Let the "Saving…" state paint before the (blocking) full-size render.
  await new Promise((res) => requestAnimationFrame(() => setTimeout(res)));
  const { data, w, h } = pipeline.renderFull(edits);
  const options = {
    source: r.path,
    width: w,
    height: h,
    target: r.target === "as" ? "path" : r.target,
    dest,
    suffix: r.suffix,
    format,
    quality: null,
    stripLocation: r.stripLocation,
    stripMetadata: r.stripMetadata,
    // The pixels are in the original's color space unless it had an unusual profile, in which
    // case they were converted to sRGB and the profile must not be attached.
    keepIcc: r.info.colorSpace !== "other",
  };
  return invoke<string>("image_save", data, { headers: { "x-save": encodeURIComponent(JSON.stringify(options)) } });
}
