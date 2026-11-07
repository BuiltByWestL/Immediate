// Editing session for the open photo: the edits, undo/redo history and "unsaved" state.

import { clone, isNeutral, neutralEdits, type Edits } from "./edits";
import type { Pipeline } from "./pipeline";
import type { ImageInfo } from "./save";

/** The live GPU pipeline and what Rust said about the photo (not reactive). */
export const session = {
  pipeline: null as Pipeline | null,
  info: null as ImageInfo | null,
};

export type Tab = "adjust" | "filters" | "crop" | "markup";

export const editor = $state({
  active: false,
  edits: neutralEdits(),
  /** Snapshots before each committed change. */
  past: [] as Edits[],
  future: [] as Edits[],
  /** Held to compare with the original. */
  comparing: false,
  tab: "adjust" as Tab,
  saving: false,
  /** The photo is loaded on the GPU and ready to edit. */
  ready: false,
  error: "",
});

/** The last committed state, for detecting a change while a slider is being dragged. */
let committed: Edits = neutralEdits();

export function startEditing() {
  editor.edits = neutralEdits();
  committed = neutralEdits();
  editor.past = [];
  editor.future = [];
  editor.tab = "adjust";
  editor.comparing = false;
  editor.active = true;
}

export function stopEditing() {
  editor.active = false;
  editor.comparing = false;
}

/** Records the current edits as one undo step (call when a drag ends or a button is pressed). */
export function commit() {
  if (JSON.stringify(committed) === JSON.stringify(editor.edits)) return;
  editor.past = [...editor.past.slice(-99), committed];
  editor.future = [];
  committed = clone(editor.edits);
}

/** Applies a change and records it in one step. */
export function change(fn: (e: Edits) => void) {
  fn(editor.edits);
  commit();
}

export function undo() {
  const prev = editor.past.at(-1);
  if (!prev) return;
  editor.past = editor.past.slice(0, -1);
  editor.future = [committed, ...editor.future];
  committed = clone(prev);
  editor.edits = clone(prev);
}

export function redo() {
  const next = editor.future[0];
  if (!next) return;
  editor.future = editor.future.slice(1);
  editor.past = [...editor.past, committed];
  committed = clone(next);
  editor.edits = clone(next);
}

export function revertAll() {
  change((e) => Object.assign(e, neutralEdits()));
}

/** Unsaved changes worth asking about before they're thrown away. */
export function hasChanges(): boolean {
  return editor.active && !isNeutral(editor.edits);
}
