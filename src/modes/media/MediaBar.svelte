<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Redo from "@lucide/svelte/icons/redo-2";
  import Undo from "@lucide/svelte/icons/undo-2";
  import { MOD } from "../../lib/shortcuts";
  import { cancelMediaEdit, cancelSave, saveMedia } from "./actions";
  import { fmtTime, media, redoMedia, undoMedia } from "./session.svelte";

  let menuOpen = $state(false);
  const streams = $derived((media.probe?.streams ?? []) as any[]);
  const isVideo = $derived(streams.some((s) => s.codec_type === "video" && s.disposition?.attached_pic !== 1));
  const hasSound = $derived(streams.some((s) => s.codec_type === "audio") && !media.edit.mute);
  const k = MOD === "⌘" ? "⌘" : "Ctrl+";
  const pct = $derived(Math.round(media.progress * 100));
</script>

<svelte:window onclick={() => (menuOpen = false)} />

<div class="side">
  <button type="button" class="btn" onclick={cancelMediaEdit} disabled={media.saving} title="Esc">Cancel</button>
</div>

<div class="center">
  {#if media.saving}
    <div class="progress" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} aria-label="Saving">
      <div class="bar"><div class="fill" style:width="{pct}%"></div></div>
      <span class="num">{pct}%{#if media.eta != null && media.eta > 1} · {fmtTime(media.eta)} left{/if}</span>
      <button type="button" class="btn" onclick={cancelSave}>Stop</button>
    </div>
  {:else}
    <button type="button" class="icon-btn" title="Undo ({k}Z)" aria-label="Undo" disabled={!media.past.length} onclick={undoMedia}><Undo size={17} strokeWidth={1.5} /></button>
    <button type="button" class="icon-btn" title="Redo ({k}Shift+Z)" aria-label="Redo" disabled={!media.future.length} onclick={redoMedia}><Redo size={17} strokeWidth={1.5} /></button>
  {/if}
</div>

<div class="side end">
  <div class="split">
    <button type="button" class="btn primary main" disabled={media.saving} onclick={() => saveMedia("copy")} title="Save a copy next to the original ({k}S)">
      {media.saving ? "Saving…" : "Save"}
    </button>
    <button
      type="button"
      class="btn primary more"
      aria-label="More ways to save"
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      disabled={media.saving}
      onclick={(e) => {
        e.stopPropagation();
        menuOpen = !menuOpen;
      }}
    >
      <ChevronDown size={16} strokeWidth={1.75} />
    </button>
    {#if menuOpen}
      <div class="menu" role="menu">
        <button type="button" role="menuitem" onclick={() => saveMedia("as")}><span>Save As…</span><span class="dim">{k}Shift+S</span></button>
        <button type="button" role="menuitem" onclick={() => saveMedia("replace")}><span>Replace original…</span></button>
        {#if isVideo}
          <div class="sep" role="separator"></div>
          {#if hasSound}<button type="button" role="menuitem" onclick={() => saveMedia("copy", "audio")}><span>Save sound only</span></button>{/if}
          <button type="button" role="menuitem" onclick={() => saveMedia("copy", "gif")}><span>Save as GIF</span></button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .side {
    display: flex;
    align-items: center;
    gap: var(--s1);
  }
  .end {
    justify-content: flex-end;
  }
  .center {
    display: flex;
    align-items: center;
    gap: var(--s1);
  }
  .progress {
    display: flex;
    align-items: center;
    gap: var(--s3);
  }
  .bar {
    width: 180px;
    height: 4px;
    border-radius: 2px;
    background: var(--hover);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 200ms linear;
  }
  .split {
    position: relative;
    display: flex;
  }
  .main {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
    min-width: 72px;
  }
  .more {
    padding: 0 var(--s1);
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    border-left: 1px solid color-mix(in srgb, var(--on-accent) 25%, transparent);
  }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 40;
    min-width: 220px;
    padding: var(--s1);
    background: var(--raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.18);
  }
  .menu button {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    height: 30px;
    padding: 0 var(--s2);
    border: 0;
    border-radius: calc(var(--radius) - 2px);
    background: transparent;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--hover);
  }
  .sep {
    height: 1px;
    margin: var(--s1) 0;
    background: var(--line);
  }
</style>
