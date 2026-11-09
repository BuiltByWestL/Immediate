// Entering and leaving the photo editor, and saving.

import { invoke } from "@tauri-apps/api/core";
import { ask, openPaths, toast, ui } from "../../lib/app.svelte";
import { editor, hasChanges, session, startEditing, stopEditing } from "./editor.svelte";
import { renderFinal, saveImage, type SaveTarget } from "./save";

/** Copies the edited photo to the clipboard. */
export async function copyEdited() {
  const p = session.pipeline;
  if (!p || !editor.ready) return;
  try {
    const { data, w, h } = renderFinal(p, editor.edits);
    await invoke("copy_pixels", data, { headers: { "x-size": `${w},${h}` } });
    toast("Copied the edited picture");
  } catch (e) {
    toast(String(e), "error");
  }
}

export function beginEdit() {
  if (!ui.file || ui.file.kind !== "image") return;
  startEditing();
}

/** Leaves the editor; asks first if there are edits. Resolves false if the user keeps editing. */
export async function cancelEdit(): Promise<boolean> {
  if (hasChanges()) {
    const discard = await ask({
      title: "Discard your edits?",
      body: "The photo stays as it was.",
      ok: "Discard",
      cancel: "Keep editing",
      danger: true,
    });
    if (!discard) return false;
  }
  stopEditing();
  return true;
}

export async function save(target: SaveTarget) {
  const file = ui.file;
  const pipeline = session.pipeline;
  const info = session.info;
  if (!file || !pipeline || !info || editor.saving) return;
  // A pasted image lives in a temp folder: ask where it should go.
  if (file.temporary && target !== "as") target = "as";
  if (target === "replace") {
    const sameFormat = info.saveExt === file.ext;
    if (!sameFormat) {
      toast(`Replace keeps the original’s format, and Immediate can’t save .${file.ext.toUpperCase()} files. Use Save As.`, "error");
      return;
    }
    const ok = await ask({
      title: `Replace “${file.name}”?`,
      body: "The edited photo takes its place. The original goes to the Recycle Bin, so you can still get it back.",
      ok: "Replace",
      cancel: "Cancel",
      danger: true,
    });
    if (!ok) return;
  }
  editor.saving = true;
  try {
    const saved = await saveImage(pipeline, editor.edits, {
      path: file.path,
      info,
      target,
      suffix: ui.settings.saveSuffix,
      stripLocation: ui.settings.removeLocation,
      output: { ...editor.output },
    });
    if (!saved) return;
    stopEditing();
    const name = saved.split(/[\\/]/).pop();
    const savedExt = name?.split(".").pop()?.toLowerCase() ?? "";
    const converted = savedExt !== file.ext.toLowerCase() && target !== "as";
    toast(converted ? `Saved “${name}” (as ${savedExt.toUpperCase()})` : `Saved “${name}”`, undefined, {
      label: "Show in folder",
      run: () => invoke("reveal", { path: saved }),
    });
    await openPaths([saved]);
  } catch (e) {
    toast(`Couldn’t save: ${e}`, "error");
  } finally {
    editor.saving = false;
  }
}
