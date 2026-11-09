<script lang="ts">
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import DropOverlay from "./app/DropOverlay.svelte";
  import Empty from "./app/Empty.svelte";
  import InfoPanel from "./app/InfoPanel.svelte";
  import Notices from "./app/Notices.svelte";
  import Sheet from "./app/Sheet.svelte";
  import ShortcutList from "./app/ShortcutList.svelte";
  import Toolbar from "./app/Toolbar.svelte";
  import Viewer from "./app/viewer/Viewer.svelte";
  import Filmstrip from "./app/viewer/Filmstrip.svelte";
  // The editor loads the first time it's opened, keeping startup light.
  const loadEditor = () => import("./modes/image/ImageEditor.svelte");
  const loadMediaEditor = () => import("./modes/media/MediaEditor.svelte");
  import { beginMediaEdit, cancelMediaEdit, saveMedia } from "./modes/media/actions";
  import { mediaKeys } from "./modes/media/keys";
  import { cutSelection, media, mediaChanged, redoMedia, undoMedia } from "./modes/media/session.svelte";
  import { prepareToolsInBackground } from "./lib/tools.svelte";
  import { answerUnsavedQueries } from "./lib/updates.svelte";
  import Recorder from "./app/Recorder.svelte";
  import { closeRecorder, openRecorder, rec, startRecording, stopRecording, togglePause } from "./lib/record.svelte";
  import { beginEdit, cancelEdit, copyEdited, save } from "./modes/image/actions";
  import { editor, hasChanges, redo, undo } from "./modes/image/editor.svelte";
  import { canEdit } from "./lib/formats";
  import {
    ask,
    connect,
    copyImage,
    copyPath,
    go,
    goTo,
    inTauri,
    newWindow,
    openPaths,
    pasteFromClipboard,
    pickFiles,
    ui,
    updateSettings,
    viewer,
  } from "./lib/app.svelte";

  const loadSettings = () => import("./app/settings/Settings.svelte");

  onMount(() => {
    connect();
    if (!inTauri) return;
    prepareToolsInBackground();
    // Files dragged from Explorer/Finder. (Tauri delivers real paths; HTML drag events don't.)
    const unlisten = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "over") ui.dropping = true;
      else if (p.type === "leave") ui.dropping = false;
      else if (p.type === "drop") {
        ui.dropping = false;
        // Never throw away edits: files dropped on a window with edits open in a new window.
        if (hasChanges() || mediaChanged()) newWindow(p.paths);
        else {
          if (editor.active) cancelEdit();
          if (media.active) cancelMediaEdit();
          openPaths(p.paths);
        }
      }
    });
    // Closing a window with unsaved edits asks first.
    const win = getCurrentWindow();
    const unclose = win.onCloseRequested(async (e) => {
      if (rec.open && rec.phase === "recording") {
        // Closing mid-recording keeps what was recorded.
        e.preventDefault();
        await stopRecording(true);
        win.destroy();
        return;
      }
      if (!(await unsavedEdits())) return;
      e.preventDefault();
      const ok = await ask({
        title: "Close without saving?",
        body: "Your edits will be lost.",
        ok: "Close",
        cancel: "Keep editing",
        danger: true,
      });
      if (ok) win.destroy();
    });
    // An update restarts every window; each says first whether it would lose anything.
    const unanswer = answerUnsavedQueries(async () => (rec.open && rec.phase !== "ready") || (await unsavedEdits()));
    return () => {
      unlisten.then((f) => f());
      unclose.then((f) => f());
      unanswer();
    };
  });

  async function unsavedEdits() {
    const { docChanged } = await import("./modes/doc/doc.svelte");
    return hasChanges() || mediaChanged() || docChanged();
  }

  function typing(t: EventTarget | null) {
    return t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(t.tagName));
  }

  async function toggleFullscreen() {
    const w = getCurrentWindow();
    await w.setFullscreen(!(await w.isFullscreen()));
  }

  function onkeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key;

    if (ui.confirm) return;
    if (rec.open && !ui.dialog) {
      const k = key.toLowerCase();
      if (rec.phase === "ready" && (k === "r" || key === " ")) startRecording();
      else if ((rec.phase === "ready" || rec.phase === "counting") && key === "Escape") closeRecorder();
      else if (rec.phase === "recording" && (key === " " || key === "Enter")) stopRecording(true);
      else if (rec.phase === "recording" && k === "p") togglePause();
      else return;
      e.preventDefault();
      return;
    }
    if (media.active && !ui.dialog) {
      const k = key.toLowerCase();
      if (mod) {
        if (k === "z" && !e.shiftKey) undoMedia();
        else if ((k === "z" && e.shiftKey) || k === "y") redoMedia();
        else if (k === "s") saveMedia(e.shiftKey ? "as" : "copy");
        else if (k === "w") getCurrentWindow().close();
        else if (k === ",") ui.dialog = "settings";
        else return;
        e.preventDefault();
        return;
      }
      if (typing(e.target)) return;
      if (key === "Escape") cancelMediaEdit();
      else if ((key === "Delete" || key === "Backspace") && media.selection) cutSelection();
      else if (!mediaKeys.handler?.(e)) return;
      e.preventDefault();
      return;
    }
    if (mod && editor.active) {
      const k = key.toLowerCase();
      if (k === "z" && !e.shiftKey) undo();
      else if ((k === "z" && e.shiftKey) || k === "y") redo();
      else if (k === "s") save(e.shiftKey ? "as" : "copy");
      else if (k === "c" && !e.shiftKey && !typing(e.target) && !window.getSelection()?.toString()) copyEdited();
      else if (["=", "+", "-", "0"].includes(k)) viewer.command?.(k === "-" ? "out" : k === "0" ? "fit" : "in");
      else if (k === ",") ui.dialog = "settings";
      else if (k === "w") getCurrentWindow().close();
      else return;
      e.preventDefault();
      return;
    }
    if (editor.active && !ui.dialog && !typing(e.target)) {
      if (key === "\\") editor.comparing = true;
      else if (key === "Escape") cancelEdit();
      else if (key === "0") viewer.command?.("fit");
      else if (key === "1") viewer.command?.("actual");
      else return;
      e.preventDefault();
      return;
    }
    if (mod) {
      const k = key.toLowerCase();
      if (k === "o") pickFiles();
      else if (k === "r" && !hasChanges() && !mediaChanged()) openRecorder();
      else if (k === "v" && !typing(e.target)) pasteFromClipboard();
      else if (k === "n") newWindow();
      else if (k === "w") getCurrentWindow().close();
      else if (k === ",") ui.dialog = "settings";
      else if (k === "c" && e.shiftKey) copyPath();
      else if (k === "c" && !typing(e.target) && !window.getSelection()?.toString()) copyImage();
      else if (k === "f" && e.metaKey) toggleFullscreen();
      else if (["=", "+", "-", "0"].includes(k)) viewer.command?.(k === "-" ? "out" : k === "0" ? "fit" : "in");
      else return;
      e.preventDefault();
      return;
    }
    if (ui.dialog || typing(e.target) || e.altKey) return;
    // Native player controls handle their own keys while focused.
    const onMedia = e.target instanceof HTMLMediaElement;

    if (key === "ArrowRight" && !onMedia) go(1);
    else if (key === "ArrowLeft" && !onMedia) go(-1);
    else if (key === "Home") goTo(0);
    else if (key === "End") goTo(ui.list.length - 1);
    else if (key === "0") viewer.command?.("fit");
    else if (key === "1") viewer.command?.("actual");
    else if (key === "+" || key === "=") viewer.command?.("in");
    else if (key === "-") viewer.command?.("out");
    else if (key === " " && viewer.media && !onMedia) {
      viewer.media.paused ? viewer.media.play() : viewer.media.pause();
    } else if (key.toLowerCase() === "i" && ui.file) updateSettings({ infoOpen: !ui.settings.infoOpen });
    else if (key.toLowerCase() === "e" && ui.file && canEdit(ui.file)) ui.file.kind === "image" ? beginEdit() : beginMediaEdit();
    else if (key === "?") ui.dialog = "shortcuts";
    else if (key === "F11") toggleFullscreen();
    else if (key === "Escape") getCurrentWindow().setFullscreen(false);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window
  {onkeydown}
  onkeyup={(e) => e.key === "\\" && (editor.comparing = false)}
  oncontextmenu={(e) => !typing(e.target) && e.preventDefault()} />

<div class="app">
  <Toolbar />
  <div class="main">
    {#if rec.open}
      <Recorder />
    {:else if ui.file && media.active}
      {#key ui.file.path}
        {#await loadMediaEditor() then m}
          <m.default file={ui.file} />
        {/await}
      {/key}
    {:else if ui.file && editor.active}
      {#key ui.file.path}
        {#await loadEditor() then m}
          <m.default file={ui.file} />
        {/await}
      {/key}
    {:else if ui.file}
      <div class="column">
        <Viewer file={ui.file} />
        {#if ui.settings.filmstrip && ui.list.length > 1}
          <Filmstrip />
        {/if}
      </div>
      {#if ui.settings.infoOpen}
        <InfoPanel file={ui.file} />
      {/if}
    {:else}
      <Empty />
    {/if}
  </div>
</div>

<DropOverlay />
<Notices />

{#if ui.confirm}
  {@const c = ui.confirm}
  <Sheet title={c.title} width={400} onclose={() => c.resolve(false)}>
    <p class="confirm-body">{c.body}</p>
    <div class="confirm-actions">
      <button type="button" class="btn lg" onclick={() => c.resolve(false)}>{c.cancel}</button>
      <button type="button" class="btn lg primary" class:danger={c.danger} onclick={() => c.resolve(true)}>{c.ok}</button>
    </div>
  </Sheet>
{/if}

{#if ui.dialog === "settings"}
  {#await loadSettings() then m}
    <m.default />
  {/await}
{:else if ui.dialog === "shortcuts"}
  <Sheet title="Keyboard shortcuts" width={460} onclose={() => (ui.dialog = null)}>
    <ShortcutList />
  </Sheet>
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .main {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .column {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .confirm-body {
    color: var(--text-dim);
  }
  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--s2);
    margin-top: var(--s5);
  }
  .danger {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
  :global([data-scheme="dark"]) .danger {
    color: #161616;
  }
</style>
