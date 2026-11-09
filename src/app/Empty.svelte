<script lang="ts">
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Mic from "@lucide/svelte/icons/mic";
  import FileText from "@lucide/svelte/icons/file-text";
  import Image from "@lucide/svelte/icons/image";
  import Music from "@lucide/svelte/icons/music";
  import Video from "@lucide/svelte/icons/video";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { inTauri, openPaths, pasteFromClipboard, pickFiles } from "../lib/app.svelte";
  import { openRecorder } from "../lib/record.svelte";

  type Recent = { path: string; name: string; dir: string; kind: "image" | "video" | "audio" | "document" | null };
  let recent = $state<Recent[]>([]);
  onMount(() => {
    if (inTauri) invoke<Recent[]>("recent_list").then((l) => (recent = l.slice(0, 8))).catch(() => {});
  });
  const ICONS = { image: Image, video: Video, audio: Music, document: FileText };
  async function clearRecent() {
    await invoke("recent_clear").catch(() => {});
    recent = [];
  }

  const mod = window.__IMMEDIATE__?.platform === "macos" ? "⌘" : "Ctrl+";
</script>

<!-- The whole area is a click target ("click to open"); the buttons are the accessible way in. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="empty" onclick={(e) => e.target === e.currentTarget && pickFiles()}>
  <div class="prompt">
    <p class="title">Drop a file here, or click to open</p>
    <p class="hint dim">Photos, videos, audio and documents</p>
    <div class="actions">
      <button type="button" class="btn lg primary" onclick={pickFiles} title="{mod}O">
        <FolderOpen size={16} strokeWidth={1.5} />Open file
      </button>
      <button type="button" class="btn lg" onclick={pasteFromClipboard} title="{mod}V">
        <ClipboardPaste size={16} strokeWidth={1.5} />Paste
      </button>
      <button type="button" class="btn lg" onclick={openRecorder} title="{mod}R">
        <Mic size={16} strokeWidth={1.5} />Record audio
      </button>
    </div>
    {#if recent.length}
      <div class="recent">
        <div class="recent-head">
          <h2>Recent</h2>
          <button type="button" class="link dim" onclick={clearRecent}>Clear</button>
        </div>
        <ul>
          {#each recent as f (f.path)}
            {@const Icon = ICONS[f.kind ?? "document"]}
            <li>
              <button type="button" class="item" title={f.path} onclick={() => openPaths([f.path])}>
                <Icon size={16} strokeWidth={1.5} />
                <span class="name">{f.name}</span>
                <span class="dir dim">{f.dir}</span>
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
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
  .recent {
    width: min(520px, 90vw);
    margin: var(--s8, 40px) auto 0;
    text-align: left;
  }
  .recent-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: var(--s2);
  }
  h2 {
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
    color: var(--text-dim);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--s2);
    width: 100%;
    min-height: 32px;
    padding: 0 var(--s2);
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .item:hover {
    background: var(--hover);
  }
  .name {
    flex: none;
    max-width: 55%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dir {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
    font-size: var(--text-sm);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
  }
</style>
