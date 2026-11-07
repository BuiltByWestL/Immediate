// Loading a picture's pixels, whoever decodes it: the webview (JPEG, PNG, WebP, AVIF, GIF, BMP)
// or Immediate's own decoders in Rust (TIFF, RAW, HEIC, PSD, JPEG XL, EXR…).

import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/** Upright pixels from Rust: width, height (u32 LE), then sRGB RGBA. */
export async function decodeWithRust(path: string): Promise<ImageData> {
  const buf = await invoke<ArrayBuffer>("decode_image", { path });
  const head = new DataView(buf, 0, 8);
  const w = head.getUint32(0, true);
  const h = head.getUint32(4, true);
  return new ImageData(new Uint8ClampedArray(buf, 8, w * h * 4), w, h);
}

export async function loadBitmap(path: string, rust: boolean, opts: ImageBitmapOptions = {}): Promise<ImageBitmap> {
  if (rust) return createImageBitmap(await decodeWithRust(path), { premultiplyAlpha: "none", ...opts, imageOrientation: "none" });
  const blob = await (await fetch(convertFileSrc(path))).blob();
  return createImageBitmap(blob, { imageOrientation: "from-image", premultiplyAlpha: "none", ...opts });
}
