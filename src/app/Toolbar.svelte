<script lang="ts">
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Info from "@lucide/svelte/icons/info";
  import Settings from "@lucide/svelte/icons/settings";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import { go, pickFiles, ui, updateSettings, viewer } from "../lib/app.svelte";
  import { canEdit } from "../lib/formats";
  import { beginEdit } from "../modes/image/actions";
  const loadEditBar = () => import("../modes/image/EditBar.svelte");
  import { editor } from "../modes/image/editor.svelte";

  const mod = window.__IMMEDIATE__?.platform === "macos" ? "⌘" : "Ctrl+";
  const zoomText = $derived(viewer.zoom == null ? "" : `${Math.round(viewer.zoom * 100)}%`);
</script>

<header class="toolbar">
  {#if editor.active}
    {#await loadEditBar() then m}
      <m.default />
    {/await}
  {:else}
  <div class="side">
    {#if ui.file}
      <button type="button" class="icon-btn" title="Previous (←)" aria-label="Previous file" disabled={ui.index <= 0} onclick={() => go(-1)}>
        <ChevronLeft size={18} strokeWidth={1.5} />
      </button>
      <button type="button" class="icon-btn" title="Next (→)" aria-label="Next file" disabled={ui.index >= ui.list.length - 1} onclick={() => go(1)}>
        <ChevronRight size={18} strokeWidth={1.5} />
      </button>
      {#if ui.list.length > 1}
        <span class="count dim num">{ui.index + 1} of {ui.list.length}</span>
      {/if}
    {/if}
  </div>

  <div class="center">
    {#if zoomText}
      <button type="button" class="zoom num" title="Fit to window (0) · Actual size (1)" onclick={() => viewer.command?.(viewer.zoom === 1 ? "fit" : "actual")}>
        {zoomText}
      </button>
    {/if}
  </div>

  <div class="side end">
    {#if ui.file}
      <button type="button" class="icon-btn" title="Open… ({mod}O)" aria-label="Open file" onclick={pickFiles}>
        <FolderOpen size={17} strokeWidth={1.5} />
      </button>
      <button
        type="button"
        class="icon-btn"
        title="Info (I)"
        aria-label="Info"
        aria-pressed={ui.settings.infoOpen}
        onclick={() => updateSettings({ infoOpen: !ui.settings.infoOpen })}
      >
        <Info size={17} strokeWidth={1.5} />
      </button>
    {/if}
    <button type="button" class="icon-btn" title="Settings ({mod},)" aria-label="Settings" onclick={() => (ui.dialog = "settings")}>
      <Settings size={17} strokeWidth={1.5} />
    </button>
    {#if ui.file && canEdit(ui.file)}
      <button type="button" class="btn primary edit" title="Edit (E)" onclick={beginEdit}>
        <SlidersHorizontal size={15} strokeWidth={1.75} />Edit
      </button>
    {/if}
  </div>
  {/if}
</header>

<style>
  .toolbar {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    height: var(--toolbar-h);
    padding: 0 var(--s2);
    background: var(--bg);
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  .side {
    display: flex;
    align-items: center;
    gap: var(--s1);
    min-width: 0;
  }
  .end {
    justify-content: flex-end;
  }
  .edit {
    margin-left: var(--s2);
  }
  .count {
    margin-left: var(--s2);
    white-space: nowrap;
  }
  .zoom {
    height: 24px;
    min-width: 52px;
    padding: 0 var(--s2);
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
  }
  .zoom:hover {
    background: var(--hover);
    color: var(--text);
  }
</style>
