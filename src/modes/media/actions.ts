// Entering and leaving the video/audio editor, and saving.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { ask, openPaths, toast, ui } from "../../lib/app.svelte";
import { ensureTools } from "../../lib/tools.svelte";
import { keptSegments, media, mediaChanged, snappedSegments, soundOf, startMediaEdit, stopMediaEdit } from "./session.svelte";

export async function beginMediaEdit() {
  const file = ui.file;
  if (!file || file.kind === "image") return;
  if (!(await ensureTools())) return;
  try {
    const probe = await invoke<any>("media_probe", { path: file.path });
    const duration = Number(probe?.format?.duration);
    if (!isFinite(duration) || duration <= 0) {
      toast("Immediate can’t tell how long this file is, so it can’t be edited.", "error");
      return;
    }
    startMediaEdit(probe, duration);
    media.keyframes = [];
    invoke<number[]>("media_keyframes", { path: file.path })
      .then((k) => (media.keyframes = k))
      .catch(() => {});
  } catch (e) {
    toast(`Couldn’t read this file: ${e}`, "error");
  }
}

export async function cancelMediaEdit(): Promise<boolean> {
  if (mediaChanged()) {
    const discard = await ask({
      title: "Discard your edits?",
      body: "The file stays as it was.",
      ok: "Discard",
      cancel: "Keep editing",
      danger: true,
    });
    if (!discard) return false;
  }
  stopMediaEdit();
  return true;
}

function extOf(p: string) {
  return (p.split(".").pop() ?? "").toLowerCase();
}

/** Video formats the picture can move to without re-encoding (mirrors convert.rs). */
function containersFor(videoCodec: string): string[] {
  const out: string[] = [];
  if (["h264", "hevc", "av1", "vp9", "mpeg4", "mpeg2video", "mpeg1video"].includes(videoCodec)) out.push("mp4");
  if (["h264", "hevc", "av1", "vp9", "mpeg4", "mpeg2video", "prores", "mjpeg", "dnxhd", "png", "qtrle"].includes(videoCodec)) out.push("mov");
  out.push("mkv");
  if (["vp8", "vp9", "av1"].includes(videoCodec)) out.push("webm");
  return out;
}

/** "": the edited file. "audio": only its sound. "gif" / "webp": an animated picture. */
export type MediaOutput = "" | "audio" | "gif" | "webp";

export async function saveMedia(target: "copy" | "as" | "replace", output: MediaOutput = "") {
  const file = ui.file;
  if (!file || media.saving) return;
  const kept = keptSegments();
  if (!kept.length) {
    toast("Nothing is left to save: every part was cut out.", "error");
    return;
  }
  const video = (media.probe?.streams ?? []).find((s: any) => s.codec_type === "video" && s.disposition?.attached_pic !== 1);
  const hasVideo = !!video;
  // GIFs and a video's sound go next to the original (the saver picks the sound's file type).
  if (output) target = file.temporary ? "as" : "copy";
  // Settings → Audio → "Save audio as" (a copy of an audio file in that format).
  const format = !hasVideo && !output && target !== "replace" && media.mode !== "smaller" && ui.settings.audioFormat ? ui.settings.audioFormat : null;
  const outExt =
    output === "gif" || output === "webp" ? output : output === "audio" ? "m4a" : media.mode === "smaller" ? (hasVideo ? "mp4" : "m4a") : (format ?? file.ext);
  // Lossless parts start on keyframes, exactly as the timeline shows.
  const segments = media.mode === "lossless" && hasVideo ? snappedSegments(kept, media.keyframes) : kept;
  let dest: string | null = null;
  if (file.temporary && target !== "as") target = "as";
  if (target === "as") {
    const base = file.path.slice(0, file.path.length - file.ext.length - 1);
    // Audio can be saved as another format by picking it here, and video moved to another
    // container (the picture is copied as it is).
    const others =
      output === "gif"
        ? []
        : !hasVideo || output === "audio"
          ? media.mode === "smaller" && !output
            ? []
            : ["wav", "flac", "mp3", "m4a", "opus", "ogg", "aiff"]
          : media.mode === "smaller"
            ? []
            : containersFor(video.codec_name);
    const choices = [outExt, ...others.filter((x) => x !== outExt)];
    dest = await saveDialog({
      defaultPath: `${base}${ui.settings.saveSuffix}.${outExt}`,
      filters: choices.map((x) => ({ name: x.toUpperCase(), extensions: [x] })),
    });
    if (!dest) return;
    if (!choices.includes(extOf(dest))) dest = `${dest}.${outExt}`;
  }
  if (target === "replace") {
    if (outExt !== file.ext) {
      toast("Replace keeps the original’s format. Use Save As for a smaller MP4.", "error");
      return;
    }
    const ok = await ask({
      title: `Replace “${file.name}”?`,
      body: "The edited version takes its place. The original goes to the Recycle Bin, so you can still get it back.",
      ok: "Replace",
      cancel: "Cancel",
      danger: true,
    });
    if (!ok) return;
  }
  media.saving = true;
  media.progress = 0;
  media.eta = null;
  const unlisten = await listen<{ progress: number; eta: number | null }>("export", (e) => {
    media.progress = e.payload.progress;
    media.eta = e.payload.eta;
  });
  try {
    const saved = await invoke<string>("media_export", {
      req: {
        source: file.path,
        sourceExt: file.ext,
        segments,
        mode: media.mode,
        mute: media.edit.mute,
        ...soundOf(),
        fadeIn: media.edit.fadeIn,
        fadeOut: media.edit.fadeOut,
        normalize: media.edit.normalize,
        normalizePeak: media.edit.normalizePeak,
        reverse: media.edit.reverse,
        channels: media.edit.channels,
        rotate: media.edit.rotate,
        flip: media.edit.flip,
        crop: media.edit.crop,
        speed: media.edit.speed,
        keepPitch: media.edit.keepPitch,
        keepAudio: media.edit.keepAudio,
        keepSubs: media.edit.keepSubs,
        subtitleFile: media.edit.subFile,
        tags: media.edit.tags,
        cover: media.edit.cover,
        sampleRate: media.edit.sampleRate,
        bitDepth: media.edit.bitDepth,
        coverFrame: media.edit.coverFrame,
        hardware: ui.settings.hardwareEncode,
        format: target === "copy" ? format : null,
        output,
        target: target === "as" ? "path" : target,
        dest,
        suffix: ui.settings.saveSuffix,
        sizeMb: media.sizeMb,
        probe: media.probe,
      },
    });
    const name = saved.split(/[\\/]/).pop();
    toast(`Saved “${name}”`, undefined, { label: "Show in folder", run: () => invoke("reveal", { path: saved }) });
    // A GIF or the sound alone is a by-product: keep editing the video.
    if (output) return;
    stopMediaEdit();
    await openPaths([saved]);
  } catch (e) {
    if (String(e) !== "cancelled") toast(String(e), "error");
  } finally {
    unlisten();
    media.saving = false;
  }
}

export function cancelSave() {
  invoke("media_cancel").catch(() => {});
}

/** Saves subtitle track `index` as an .srt next to the video, timed to the edit. */
export async function saveSubtitles(index: number) {
  const file = ui.file;
  if (!file) return;
  try {
    const saved = await invoke<string>("media_save_subtitles", { path: file.path, index, segments: keptSegments(), speed: media.edit.speed, probe: media.probe });
    toast(`Saved “${saved.split(/[\\/]/).pop()}”`, undefined, { label: "Show in folder", run: () => invoke("reveal", { path: saved }) });
  } catch (e) {
    toast(String(e), "error");
  }
}

export async function grabFrame() {
  const file = ui.file;
  if (!file) return;
  try {
    const saved = await invoke<string>("media_grab_frame", { path: file.path, time: media.time });
    toast(`Saved “${saved.split(/[\\/]/).pop()}”`, undefined, { label: "Show in folder", run: () => invoke("reveal", { path: saved }) });
  } catch (e) {
    toast(String(e), "error");
  }
}
