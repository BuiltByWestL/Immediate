// Quick recording: microphone → WAV in Music/Immediate Recordings → opened in the editor.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openPaths, toast, ui, updateSettings } from "./app.svelte";

export const rec = $state({
  open: false,
  phase: "ready" as "ready" | "counting" | "recording" | "stopping",
  /** Seconds left in the count-in. */
  count: 0,
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
  // Settings → Audio → Microphone, else the last one used here, else the system's default.
  const preferred = ui.settings.recordDevice;
  if (preferred && rec.devices.includes(preferred)) rec.device = preferred;
  else if (!rec.devices.includes(rec.device)) rec.device = rec.devices[0] ?? "";
}

export function closeRecorder() {
  cancelCountIn();
  if (rec.phase === "recording") return;
  rec.open = false;
}

function name() {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `Recording ${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}.${p(d.getMinutes())}.${p(d.getSeconds())}`;
}

let counting: ReturnType<typeof setInterval> | null = null;

/** Starts recording, after a 3-2-1 count-in when that's on (Settings → Audio, or here). */
export async function startRecording() {
  if (rec.phase !== "ready") return;
  if (ui.settings.countIn) {
    rec.phase = "counting";
    rec.count = 3;
    counting = setInterval(() => {
      rec.count--;
      if (rec.count > 0) return;
      cancelCountIn();
      record();
    }, 1000);
    return;
  }
  await record();
}

/** Stops a count-in before recording starts. */
export function cancelCountIn() {
  if (counting) clearInterval(counting);
  counting = null;
  if (rec.phase === "counting") rec.phase = "ready";
}

async function record() {
  if (rec.phase !== "ready") return;
  unlisten ??= await listen<{ peak: number; seconds: number; paused: boolean }>("rec", (e) => {
    rec.peak = e.payload.peak;
    rec.seconds = e.payload.seconds;
    if (!e.payload.paused) rec.history = [...rec.history.slice(-299), e.payload.peak];
  });
  try {
    await invoke("rec_start", { device: rec.device || null, name: name() });
    // A microphone picked here becomes the one to start with next time.
    if (rec.device && !rec.device.startsWith("System sound: ") && rec.device !== ui.settings.recordDevice && ui.settings.recordDevice !== "") {
      updateSettings({ recordDevice: rec.device });
    }
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
