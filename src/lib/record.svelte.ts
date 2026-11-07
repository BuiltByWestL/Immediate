// Quick recording: microphone → WAV in Music/Immediate Recordings → opened in the editor.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openPaths, toast } from "./app.svelte";

export const rec = $state({
  open: false,
  phase: "ready" as "ready" | "recording" | "stopping",
  paused: false,
  devices: [] as string[],
  device: "",
  peak: 0,
  seconds: 0,
  /** Recent levels for the live meter strip (newest last). */
  history: [] as number[],
});

let unlisten: (() => void) | null = null;

export async function openRecorder() {
  rec.open = true;
  rec.phase = "ready";
  rec.paused = false;
  rec.seconds = 0;
  rec.history = [];
  rec.devices = await invoke<string[]>("rec_devices").catch(() => []);
  if (!rec.devices.includes(rec.device)) rec.device = rec.devices[0] ?? "";
}

export function closeRecorder() {
  if (rec.phase === "recording") return;
  rec.open = false;
}

function name() {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `Recording ${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}.${p(d.getMinutes())}.${p(d.getSeconds())}`;
}

export async function startRecording() {
  if (rec.phase !== "ready") return;
  unlisten ??= await listen<{ peak: number; seconds: number; paused: boolean }>("rec", (e) => {
    rec.peak = e.payload.peak;
    rec.seconds = e.payload.seconds;
    if (!e.payload.paused) rec.history = [...rec.history.slice(-299), e.payload.peak];
  });
  try {
    await invoke("rec_start", { device: rec.device || null, name: name() });
    rec.phase = "recording";
    rec.paused = false;
  } catch (e) {
    toast(String(e), "error");
  }
}

export function togglePause() {
  rec.paused = !rec.paused;
  invoke("rec_pause", { paused: rec.paused });
}

/** Stops; keeps the file and opens it for trimming (or discards it). */
export async function stopRecording(keep: boolean) {
  if (rec.phase !== "recording") return;
  rec.phase = "stopping";
  try {
    const path = await invoke<string>("rec_stop", { keep });
    rec.open = false;
    rec.phase = "ready";
    if (keep && path) {
      toast(`Saved “${path.split(/[\\/]/).pop()}” in Music › Immediate Recordings`, undefined, {
        label: "Show in folder",
        run: () => invoke("reveal", { path }),
      });
      await openPaths([path]);
      const { beginMediaEdit } = await import("../modes/media/actions");
      await beginMediaEdit();
    }
  } catch (e) {
    rec.phase = "ready";
    toast(String(e), "error");
  }
}
