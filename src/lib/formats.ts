import formats from "../formats.json";
import type { OpenedFile } from "./types";

type Group = Record<string, string>;
type Entry = { edit: Group; view: Group };
const kinds = { image: formats.image as Entry, video: formats.video as Entry, audio: formats.audio as Entry };

/** Every extension Immediate opens (for the Open dialog). */
export const ALL_EXTENSIONS = Object.values(kinds).flatMap((k) => [...Object.keys(k.edit), ...Object.keys(k.view)]);

// What the system webview can show by itself. Everything else needs Immediate's own decoders
// (images, P2) or media tools (video/audio proxies, P3). Video/audio in these containers may still
// use a codec the webview lacks; the viewer then says so (see MediaView).
const NATIVE: Record<string, Set<string>> = {
  image: new Set(["jpg", "jpeg", "jfif", "png", "apng", "gif", "webp", "bmp", "ico", "avif", "svg"]),
  video: new Set(["mp4", "m4v", "mov", "webm", "mkv", "ogv"]),
  audio: new Set(["mp3", "wav", "bwf", "flac", "aac", "m4a", "m4b", "ogg", "oga", "opus"]),
};

/** Pictures Immediate decodes itself (src-tauri/src/image/decode.rs). */
const DECODED = new Set([
  "tif", "tiff", "tga", "pnm", "ppm", "pgm", "pbm", "qoi", "hdr", "exr", "dds", "psd", "psb", "jxl", "heic", "heif",
  "dng", "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "pef", "srw", "3fr", "iiq",
]);

export function needsDecoding(f: OpenedFile): boolean {
  return f.kind === "image" && !NATIVE.image.has(f.ext) && DECODED.has(f.ext);
}

export function canPreview(f: OpenedFile): boolean {
  return (NATIVE[f.kind]?.has(f.ext) ?? false) || needsDecoding(f);
}

/** Pictures the editor opens (vector and icon files are view only). */
export function canEdit(f: OpenedFile): boolean {
  return f.kind === "image" && f.ext !== "svg" && f.ext !== "ico" && canPreview(f);
}

export function isNativeImagePath(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return NATIVE.image.has(ext);
}

const KIND_NAMES = { image: "Image", video: "Video", audio: "Audio" };
export function kindName(f: OpenedFile) {
  return KIND_NAMES[f.kind];
}

/** "4.2 MB" (decimal units, like Finder and most people expect). */
export function formatSize(bytes: number): string {
  if (bytes < 1000) return `${bytes} bytes`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes;
  let i = -1;
  do {
    v /= 1000;
    i++;
  } while (v >= 1000 && i < units.length - 1);
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

export function formatDate(ms: number | null | undefined): string {
  if (!ms) return "";
  return new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** "1:05", "1:02:05" */
export function formatDuration(s: number): string {
  if (!isFinite(s)) return "";
  const t = Math.round(s);
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const sec = String(t % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}
