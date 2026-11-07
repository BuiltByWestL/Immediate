// Text-based documents (txt, md, csv, json…): the open text, unsaved state, and Save.

import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { ask, openPaths, toast, ui } from "../../lib/app.svelte";

export const doc = $state({
  path: "",
  /** Text as loaded (to detect and revert changes). */
  original: "",
  text: "",
  encoding: "utf-8",
  eol: "\n",
  loading: false,
  error: "",
  saving: false,
});

export const docChanged = () => !!doc.path && doc.text !== doc.original;

export async function loadText(path: string) {
  doc.path = path;
  doc.loading = true;
  doc.error = "";
  try {
    const t = await invoke<{ text: string; encoding: string; eol: string }>("read_text", { path });
    if (doc.path !== path) return;
    doc.original = doc.text = t.text;
    doc.encoding = t.encoding;
    doc.eol = t.eol;
  } catch (e) {
    doc.error = String(e);
  } finally {
    doc.loading = false;
  }
}

export function closeDoc() {
  doc.path = "";
  doc.text = doc.original = "";
}

export function revertDoc() {
  doc.text = doc.original;
}

/** Asks before discarding unsaved text. */
export async function confirmLeaveDoc(): Promise<boolean> {
  if (!docChanged()) return true;
  return ask({ title: "Discard your changes?", body: "The file stays as it was.", ok: "Discard", cancel: "Keep editing", danger: true });
}

export async function saveDoc(target: "copy" | "as" | "replace", text = doc.text, ext?: string) {
  const file = ui.file;
  if (!file || doc.saving) return;
  let dest: string | null = null;
  const outExt = ext ?? file.ext;
  if (target === "as" || outExt !== file.ext) {
    const base = file.path.slice(0, file.path.length - file.ext.length - 1);
    dest = await saveDialog({ defaultPath: `${base}${ext ? "" : ui.settings.saveSuffix}.${outExt}`, filters: [{ name: outExt.toUpperCase(), extensions: [outExt] }] });
    if (!dest) return;
    target = "as";
  }
  if (target === "replace") {
    const ok = await ask({
      title: `Replace “${file.name}”?`,
      body: "Your changes are saved into it. The previous version goes to the Recycle Bin.",
      ok: "Replace",
      cancel: "Cancel",
      danger: true,
    });
    if (!ok) return;
  }
  doc.saving = true;
  try {
    const saved = await invoke<string>("save_text", {
      req: {
        source: file.path,
        text,
        encoding: ext ? "utf-8" : doc.encoding,
        eol: doc.eol,
        target: target === "as" ? "path" : target,
        dest,
        suffix: ui.settings.saveSuffix,
      },
    });
    if (!ext) doc.original = doc.text;
    toast(`Saved “${saved.split(/[\\/]/).pop()}”`, undefined, { label: "Show in folder", run: () => invoke("reveal", { path: saved }) });
    if (!ext && target !== "replace") await openPaths([saved]);
  } catch (e) {
    toast(String(e), "error");
  } finally {
    doc.saving = false;
  }
}
