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
  import {
    connect,
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
    // Files dragged from Explorer/Finder. (Tauri delivers real paths; HTML drag events don't.)
    const unlisten = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "over") ui.dropping = true;
      else if (p.type === "leave") ui.dropping = false;
      else if (p.type === "drop") {
        ui.dropping = false;
        openPaths(p.paths);
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
  });

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

    if (mod) {
      const k = key.toLowerCase();
      if (k === "o") pickFiles();
      else if (k === "v" && !typing(e.target)) pasteFromClipboard();
      else if (k === "n") newWindow();
      else if (k === "w") getCurrentWindow().close();
      else if (k === ",") ui.dialog = "settings";
      else if (k === "c" && e.shiftKey) copyPath();
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
    else if (key === "?") ui.dialog = "shortcuts";
    else if (key === "F11") toggleFullscreen();
    else if (key === "Escape") getCurrentWindow().setFullscreen(false);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window {onkeydown} oncontextmenu={(e) => !typing(e.target) && e.preventDefault()} />

<div class="app">
  <Toolbar />
  <div class="main">
    {#if ui.file}
      <Viewer file={ui.file} />
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
</style>
