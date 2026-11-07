// Entering and leaving the video/audio editor, and saving.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { ask, openPaths, toast, ui } from "../../lib/app.svelte";
import { ensureTools } from "../../lib/tools.svelte";
import { keptSegments, media, mediaChanged, snappedSegments, startMediaEdit, stopMediaEdit } from "./session.svelte";

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

export async function saveMedia(target: "copy" | "as" | "replace") {
  const file = ui.file;
  if (!file || media.saving) return;
  const kept = keptSegments();
  if (!kept.length) {
    toast("Nothing is left to save: every part was cut out.", "error");
    return;
  }
  const hasVideo = (media.probe?.streams ?? []).some((s: any) => s.codec_type === "video" && s.disposition?.attached_pic !== 1);
  const outExt = media.mode === "smaller" ? (hasVideo ? "mp4" : "m4a") : file.ext;
  // Lossless parts start on keyframes, exactly as the timeline shows.
  const segments = media.mode === "lossless" && hasVideo ? snappedSegments(kept, media.keyframes) : kept;
  let dest: string | null = null;
  if (file.temporary && target !== "as") target = "as";
  if (target === "as") {
    const base = file.path.slice(0, file.path.length - file.ext.length - 1);
    dest = await saveDialog({ defaultPath: `${base}${ui.settings.saveSuffix}.${outExt}`, filters: [{ name: outExt.toUpperCase(), extensions: [outExt] }] });
    if (!dest) return;
    if (extOf(dest) !== outExt) dest = `${dest}.${outExt}`;
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
        segments,
        mode: media.mode,
        mute: media.edit.mute,
        gainDb: media.edit.gain,
        fadeIn: media.edit.fadeIn,
        fadeOut: media.edit.fadeOut,
        normalize: media.edit.normalize,
        target: target === "as" ? "path" : target,
        dest,
        suffix: ui.settings.saveSuffix,
        sizeMb: media.sizeMb,
        probe: media.probe,
      },
    });
    stopMediaEdit();
    const name = saved.split(/[\\/]/).pop();
    toast(`Saved “${name}”`, undefined, { label: "Show in folder", run: () => invoke("reveal", { path: saved }) });
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
