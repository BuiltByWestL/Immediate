import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getAllWindows, getCurrentWindow } from "@tauri-apps/api/window";
import { ask } from "./app.svelte";

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

// ---- unsaved work in any window ----
// Installing restarts the whole app, every window at once. Before that, each window is asked
// whether it holds work that would be lost (edits, a recording), so nothing goes silently.

interface Reply {
  query: string;
  label: string;
  title: string;
  unsaved: boolean;
}

/** Every window calls this once, with its own "would anything be lost here?" check. */
export function answerUnsavedQueries(check: () => Promise<boolean>): () => void {
  const win = getCurrentWindow();
  const asked = listen<string>("unsaved-query", async (e) => {
    const unsaved = await check().catch(() => false);
    const title = await win.title().catch(() => "");
    await emit("unsaved-reply", { query: e.payload, label: win.label, title, unsaved } satisfies Reply);
  });
  const shown = listen<string>("unsaved-show", async (e) => {
    if (e.payload !== win.label) return;
    await win.unminimize().catch(() => {});
    await win.setFocus().catch(() => {});
  });
  return () => {
    asked.then((f) => f());
    shown.then((f) => f());
  };
}

/** The windows with unsaved work, this one included. Waits for every window, at most 1.5 s. */
async function windowsWithUnsavedWork(): Promise<Reply[]> {
  const query = `${Date.now()}-${Math.random()}`;
  const replies: Reply[] = [];
  const total = (await getAllWindows().catch(() => [])).length || 1;
  let done = () => {};
  const all = new Promise<void>((r) => (done = r));
  const stop = await listen<Reply>("unsaved-reply", (e) => {
    if (e.payload.query !== query) return;
    replies.push(e.payload);
    if (replies.length >= total) done();
  });
  await emit("unsaved-query", query);
  await Promise.race([all, new Promise((r) => setTimeout(r, 1500))]);
  stop();
  return replies.filter((r) => r.unsaved);
}

function names(list: Reply[]): string {
  const t = list.map((r) => (r.title && r.title !== "Immediate" ? `“${r.title}”` : "a window"));
  if (t.length <= 1) return t[0] ?? "";
  return `${t.slice(0, -1).join(", ")} and ${t[t.length - 1]}`;
}

export async function installUpdate() {
  if (updates.phase === "installing") return;
  const here = getCurrentWindow().label;
  const unsaved = await windowsWithUnsavedWork();
  if (unsaved.length) {
    const onlyHere = unsaved.length === 1 && unsaved[0].label === here;
    const ok = await ask({
      title: "Restart without saving?",
      body: onlyHere
        ? "Immediate restarts to install the update, and your unsaved edits here would be lost. Save them first, or install later."
        : `Immediate restarts to install the update. Unsaved work in ${names(unsaved)} would be lost. Save it first, or install later.`,
      ok: "Restart anyway",
      cancel: "Keep editing",
      danger: true,
    });
    if (!ok) {
      // Bring the first other window with unsaved work forward, so it's easy to find.
      const other = unsaved.find((r) => r.label !== here);
      if (other) await emit("unsaved-show", other.label);
      return;
    }
  }
  updates.phase = "installing";
  try {
    await invoke("update_install"); // the app restarts into the new version
  } catch (e) {
    updates.phase = "error";
    updates.error = String(e);
  }
}
