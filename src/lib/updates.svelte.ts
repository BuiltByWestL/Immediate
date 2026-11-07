import { invoke } from "@tauri-apps/api/core";

export interface UpdateInfo {
  version: string;
  current_version: string;
  notes: string | null;
}

export const updates = $state({
  available: null as UpdateInfo | null,
  /** The offer was dismissed with "Later" (until the next manual check). */
  hidden: false,
  phase: "idle" as "idle" | "checking" | "installing" | "error",
  error: "",
  lastResult: "" as "" | "current" | "available" | "failed",
});

/** Quiet checks say nothing on failure; a manual check (Settings) reports it. */
export async function checkForUpdates(manual = false) {
  updates.phase = "checking";
  try {
    updates.available = await invoke<UpdateInfo | null>("update_check");
    updates.lastResult = updates.available ? "available" : "current";
    if (manual) updates.hidden = false;
    updates.phase = "idle";
  } catch (e) {
    updates.phase = "idle";
    updates.error = String(e);
    // A quiet check that fails stays quiet everywhere, Settings included.
    if (manual) updates.lastResult = "failed";
  }
}

export async function installUpdate() {
  updates.phase = "installing";
  try {
    await invoke("update_install"); // the app restarts into the new version
  } catch (e) {
    updates.phase = "error";
    updates.error = String(e);
  }
}
