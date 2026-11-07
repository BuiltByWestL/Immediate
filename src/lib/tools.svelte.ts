// Media tools (ffmpeg) for video and audio: found on this computer, or installed by Immediate in
// the background the first time they're needed. Photos never need them.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { inTauri, toast } from "./app.svelte";

export interface Tools {
  ffmpeg: string;
  ffprobe: string;
  version: string;
  managed: boolean;
}

export const tools = $state({
  /** undefined = not checked yet */
  found: undefined as Tools | null | undefined,
  installing: false,
  phase: "" as "" | "download" | "verify" | "unpack" | "done" | "error",
  received: 0,
  total: 0,
  error: "",
});

let listening = false;
async function listenProgress() {
  if (listening || !inTauri) return;
  listening = true;
  await listen<{ phase: typeof tools.phase; received: number; total: number; message: string }>("tools", (e) => {
    const p = e.payload;
    tools.phase = p.phase;
    if (p.total) tools.total = p.total;
    if (p.received) tools.received = p.received;
    if (p.phase === "error") tools.error = p.message;
  });
}

export async function checkTools(): Promise<Tools | null> {
  if (!inTauri) return null;
  tools.found = await invoke<Tools | null>("media_tools").catch(() => null);
  return tools.found;
}

let installing: Promise<boolean> | null = null;

/** Installs Immediate's own copy (also used to replace an old system copy). */
export function installTools(): Promise<boolean> {
  if (installing) return installing;
  installing = (async () => {
    await listenProgress();
    tools.installing = true;
    tools.error = "";
    tools.received = 0;
    tools.total = 0;
    try {
      await invoke("tools_install");
      await checkTools();
      return !!tools.found;
    } catch (e) {
      tools.error = String(e);
      toast(String(e), "error");
      return false;
    } finally {
      tools.installing = false;
      installing = null;
    }
  })();
  return installing;
}

/** True when tools are ready; installs them first if needed (the pill shows progress). */
export async function ensureTools(): Promise<boolean> {
  if (tools.found === undefined) await checkTools();
  if (tools.found) return true;
  return installTools();
}

/** First launch: set the tools up quietly in the background so video "just works" later. */
export function prepareToolsInBackground() {
  setTimeout(async () => {
    if ((await checkTools()) === null) installTools();
  }, 4000);
}
