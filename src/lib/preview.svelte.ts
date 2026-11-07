// Playback for video/audio the built-in player can't decode: a stand-in made with media tools
// (H.264/AAC, cached), played while every edit still applies to the original file.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ensureTools } from "./tools.svelte";
import type { OpenedFile } from "./types";

// What the webview plays itself (checked on Windows' engine; see scripts/playback-matrix.html).
const CONTAINERS = new Set(["mp4", "m4v", "mov", "webm", "mkv", "ogv", "mp3", "m4a", "m4b", "ogg", "oga", "opus", "flac", "wav", "bwf", "aac"]);
const VIDEO = new Set(["h264", "hevc", "vp8", "vp9", "av1"]);
const AUDIO = new Set(["aac", "mp3", "opus", "vorbis", "flac", "pcm_s16le", "pcm_s24le", "pcm_f32le", "pcm_u8"]);

export function nativeContainer(f: OpenedFile) {
  return CONTAINERS.has(f.ext);
}

/** Whether the file needs a stand-in to play, from what ffprobe found in it. */
export function needsPreview(f: OpenedFile, probe: any): boolean {
  if (!nativeContainer(f)) return true;
  const streams: any[] = probe?.streams ?? [];
  const v = streams.find((s) => s.codec_type === "video" && s.disposition?.attached_pic !== 1);
  const a = streams.find((s) => s.codec_type === "audio");
  return (!!v && !VIDEO.has(v.codec_name)) || (!!a && !AUDIO.has(a.codec_name));
}

export const preview = $state({ path: "", progress: 0 });

let listening = false;

/** Makes (or reuses) the stand-in and returns a URL the player can use. */
export async function previewUrl(f: OpenedFile, probe?: any): Promise<string> {
  if (!(await ensureTools())) throw new Error("Media tools aren’t available, so this file can’t play yet.");
  if (!listening) {
    listening = true;
    await listen<{ progress: number }>("proxy", (e) => (preview.progress = e.payload.progress));
  }
  probe ??= await invoke<any>("media_probe", { path: f.path });
  const duration = Number(probe?.format?.duration) || 1;
  const video = (probe?.streams ?? []).some((s: any) => s.codec_type === "video" && s.disposition?.attached_pic !== 1);
  preview.path = f.path;
  preview.progress = 0;
  try {
    const out = await invoke<string>("media_proxy", { path: f.path, duration, video });
    return convertFileSrc(out);
  } finally {
    preview.path = "";
  }
}
