// Window state: the open file, the set of files ←/→ moves through, settings, and UI overlays.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { applyTheme } from "../design/theme";
import { OPEN_FILTERS, isNativeImagePath } from "./formats";
import { DEFAULT_SETTINGS, type Batch, type Details, type MediaFacts, type OpenedFile, type Settings } from "./types";

export const inTauri = "__TAURI_INTERNALS__" in window;

export interface Toast {
  id: number;
  text: string;
  tone?: "error";
  action?: { label: string; run: () => void };
}

export const ui = $state({
  settings: { ...DEFAULT_SETTINGS, ...window.__IMMEDIATE__?.settings } as Settings,
  file: null as OpenedFile | null,
  /** Files ←/→ moves through: the file's folder, or the set of files opened together. */
  list: [] as string[],
  index: -1,
  /** true when `list` is a set of files opened together rather than a folder. */
  isSet: false,
  facts: {} as MediaFacts,
  details: null as Details | null,
  /** ffprobe JSON for video/audio (when media tools are available). */
  probe: null as any,
  loading: false,
  dialog: null as null | "settings" | "shortcuts",
  settingsSection: "general",
  dropping: false,
  toasts: [] as Toast[],
  confirm: null as null | Confirm,
});

export interface Confirm {
  title: string;
  body: string;
  ok: string;
  cancel: string;
  danger?: boolean;
  resolve: (ok: boolean) => void;
}

/** Asks a yes/no question in a small sheet. Resolves true for `ok`. */
export function ask(q: Omit<Confirm, "resolve">): Promise<boolean> {
  return new Promise((resolve) => {
    ui.confirm = {
      ...q,
      resolve: (ok) => {
        ui.confirm = null;
        resolve(ok);
      },
    };
  });
}

/** The active viewer's controls, for the toolbar and keyboard shortcuts. */
export const viewer = $state({
  /** Current photo zoom (1 = actual size), or null when not showing a photo. */
  zoom: null as number | null,
  /** "fit" | "actual" | "in" | "out"; returns false if the viewer doesn't handle it. */
  command: null as null | ((cmd: string) => boolean),
  /** The playing <video>/<audio>, for Space. */
  media: null as HTMLMediaElement | null,
});

// ---- settings ---------------------------------------------------------------------------

export async function updateSettings(patch: Partial<Settings>) {
  const next = { ...ui.settings, ...patch };
  ui.settings = next;
  applyTheme(next);
  if (!inTauri) return;
  try {
    ui.settings = await invoke<Settings>("settings_set", { settings: next });
  } catch (e) {
    toast(`Couldn’t save settings: ${e}`, "error");
  }
}

// ---- toasts -----------------------------------------------------------------------------

let toastId = 0;
export function toast(text: string, tone?: "error", action?: Toast["action"]) {
  const t: Toast = { id: ++toastId, text, tone, action };
  ui.toasts = [...ui.toasts.slice(-2), t];
  setTimeout(() => dismiss(t.id), action ? 6000 : 3200);
}
export function dismiss(id: number) {
  ui.toasts = ui.toasts.filter((t) => t.id !== id);
}

// ---- leaving a file ----

/** Checks registered by views with unsaved work; all must agree before moving to another file. */
export const leaveGuards = new Set<() => Promise<boolean>>();
async function canLeave(): Promise<boolean> {
  for (const g of leaveGuards) if (!(await g())) return false;
  return true;
}

// ---- opening files ----------------------------------------------------------------------

let openSeq = 0;

/** Shows one file. Later calls win if several are in flight (fast ←/→). */
async function show(path: string): Promise<boolean> {
  const seq = ++openSeq;
  ui.loading = true;
  try {
    const file = await invoke<OpenedFile>("open_path", { path });
    if (seq !== openSeq) return false;
    ui.file = file;
    ui.facts = {};
    ui.details = null;
    ui.probe = null;
    if (file.kind !== "image") {
      invoke("media_probe", { path: file.path })
        .then((p) => {
          if (seq === openSeq) ui.probe = p;
        })
        .catch(() => {});
    }
    getCurrentWindow().setTitle(file.name).catch(() => {});
    invoke<Details>("file_details", { path: file.path })
      .then((d) => {
        if (seq === openSeq) ui.details = d;
      })
      .catch(() => {});
    preloadNeighbors();
    return true;
  } catch (e) {
    if (seq === openSeq) toast(String(e), "error");
    return false;
  } finally {
    if (seq === openSeq) ui.loading = false;
  }
}

/** Opens files: one file browses its folder; several become a set to browse. */
/** Adds a file the user opened to the start screen's recent list. */
function remember(path: string) {
  invoke("recent_add", { path }).catch(() => {});
}

export async function openPaths(paths: string[], append = false) {
  if (!paths.length) return;
  if (!append && !(await canLeave())) return;
  if (append && ui.file) {
    const base = ui.isSet ? ui.list : [ui.file.path];
    ui.list = [...base, ...paths.filter((p) => !base.includes(p))];
    ui.index = ui.list.indexOf(ui.file.path);
    ui.isSet = true;
    return;
  }
  if (paths.length > 1) {
    ui.list = paths;
    ui.isSet = true;
    ui.index = 0;
    if (await show(paths[0])) remember(paths[0]);
    return;
  }
  if (!(await show(paths[0]))) return;
  const opened = ui.file!.path;
  remember(opened);
  ui.isSet = false;
  ui.list = [opened];
  ui.index = 0;
  try {
    const siblings = await invoke<string[]>("list_folder", { path: opened });
    if (ui.file?.path !== opened) return; // moved on meanwhile
    const i = siblings.indexOf(opened);
    if (i >= 0) {
      ui.list = siblings;
      ui.index = i;
      preloadNeighbors();
    }
  } catch {
    /* folder not readable: browse just this file */
  }
}

export async function go(delta: number) {
  const i = ui.index + delta;
  if (i < 0 || i >= ui.list.length || i === ui.index) return;
  if (!(await canLeave())) return;
  ui.index = i;
  await show(ui.list[i]);
}

export async function goTo(i: number) {
  if (i < 0 || i >= ui.list.length) return;
  if (!(await canLeave())) return;
  ui.index = i;
  await show(ui.list[i]);
}

const preloaded = new Map<string, HTMLImageElement>();
function preloadNeighbors() {
  if (ui.isSet) return; // only folder siblings are readable before they're opened
  const wanted = [ui.list[ui.index + 1], ui.list[ui.index - 1]].filter((p) => p && isNativeImagePath(p));
  for (const key of preloaded.keys()) if (!wanted.includes(key)) preloaded.delete(key);
  for (const p of wanted) {
    if (preloaded.has(p)) continue;
    const img = new Image();
    img.decoding = "async";
    img.src = convertFileSrc(p);
    img.decode().catch(() => {});
    preloaded.set(p, img);
  }
}

export async function pickFiles() {
  try {
    const picked = await openDialog({
      multiple: true,
      directory: false,
      filters: OPEN_FILTERS,
    });
    if (picked?.length) await openPaths(picked);
  } catch (e) {
    toast(String(e), "error");
  }
}

export async function pasteFromClipboard() {
  try {
    await openPaths(await invoke<string[]>("clipboard_paste"));
  } catch (e) {
    toast(String(e), "error");
  }
}

export function revealCurrent() {
  if (ui.file) invoke("reveal", { path: ui.file.path }).catch((e) => toast(String(e), "error"));
}

export async function copyPath() {
  if (!ui.file) return;
  try {
    await navigator.clipboard.writeText(ui.file.path);
    toast("Copied the file’s location");
  } catch (e) {
    toast(`Couldn’t copy: ${e}`, "error");
  }
}

/** Copies the open picture to the clipboard. */
export async function copyImage() {
  if (ui.file?.kind !== "image") return;
  try {
    await invoke("copy_image_file", { path: ui.file.path });
    toast("Copied the picture");
  } catch (e) {
    toast(String(e), "error");
  }
}

export function newWindow(files: string[] = []) {
  invoke("open_in_new_window", { files }).catch((e) => toast(String(e), "error"));
}

/**
 * Takes files waiting for this window (from the command line, Explorer, a second launch).
 * Runs one at a time, so files that arrive while the first is opening are added after it.
 */
let inboxChain = Promise.resolve();
function takeInbox() {
  inboxChain = inboxChain.then(async () => {
    const batches = await invoke<Batch[]>("inbox_take");
    for (const b of batches) await openPaths(b.files, b.append || !!ui.file);
  });
  return inboxChain;
}

export async function connect() {
  if (!inTauri) return;
  applyTheme(ui.settings);
  await listen("inbox", () => takeInbox());
  await listen<Settings>("settings", (e) => {
    ui.settings = e.payload;
    applyTheme(e.payload);
  });
  await takeInbox();
}
