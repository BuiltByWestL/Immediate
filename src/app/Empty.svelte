<script lang="ts">
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import { pasteFromClipboard, pickFiles } from "../lib/app.svelte";

  const mod = window.__IMMEDIATE__?.platform === "macos" ? "⌘" : "Ctrl+";
</script>

<!-- The whole area is a click target ("click to open"); the buttons are the accessible way in. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="empty" onclick={(e) => e.target === e.currentTarget && pickFiles()}>
  <div class="prompt">
    <p class="title">Drop a file here, or click to open</p>
    <p class="hint dim">Photos, videos and audio</p>
    <div class="actions">
      <button type="button" class="btn lg primary" onclick={pickFiles} title="{mod}O">
        <FolderOpen size={16} strokeWidth={1.5} />Open file
      </button>
      <button type="button" class="btn lg" onclick={pasteFromClipboard} title="{mod}V">
        <ClipboardPaste size={16} strokeWidth={1.5} />Paste
      </button>
    </div>
  </div>
</div>

<style>
  .empty {
    flex: 1;
    display: grid;
    place-items: center;
    background: var(--canvas);
    cursor: pointer;
  }
  .prompt {
    text-align: center;
    cursor: default;
  }
  .title {
    font-size: var(--text-lg);
    font-weight: var(--w-medium);
  }
  .hint {
    margin: var(--s1) 0 var(--s6);
  }
  .actions {
    display: flex;
    gap: var(--s2);
    justify-content: center;
  }
</style>
