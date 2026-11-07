<script lang="ts">
  // Appears when a document has changes: Revert · Save (copy) · Save As · Replace original.
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { docChanged, doc, revertDoc, saveDoc } from "./doc.svelte";

  let { children }: { children?: import("svelte").Snippet } = $props();
  let menu = $state(false);
  const changed = $derived(docChanged());
</script>

<svelte:window onclick={() => (menu = false)} />

<div class="strip">
  <div class="left">{@render children?.()}</div>
  {#if changed}
    <span class="dim small">Unsaved changes</span>
    <button type="button" class="btn" onclick={revertDoc}>Revert</button>
    <div class="split">
      <button type="button" class="btn primary main" disabled={doc.saving} onclick={() => saveDoc("copy")} title="Save a copy next to the original (Ctrl+S)">
        {doc.saving ? "Saving…" : "Save"}
      </button>
      <button type="button" class="btn primary more" aria-label="More ways to save" onclick={(e) => (e.stopPropagation(), (menu = !menu))}>
        <ChevronDown size={16} strokeWidth={1.75} />
      </button>
      {#if menu}
        <div class="menu" role="menu">
          <button type="button" role="menuitem" onclick={() => saveDoc("as")}>Save As…</button>
          <button type="button" role="menuitem" onclick={() => saveDoc("replace")}>Replace original…</button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .strip {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--s2);
    min-height: 40px;
    padding: var(--s1) var(--s3);
    background: var(--bg);
    border-bottom: 1px solid var(--line);
  }
  .left {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--s2);
    min-width: 0;
  }
  .small {
    font-size: var(--text-sm);
  }
  .split {
    position: relative;
    display: flex;
  }
  .main {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
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
    min-width: 200px;
    padding: var(--s1);
    background: var(--raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.18);
  }
  .menu button {
    display: block;
    width: 100%;
    height: 30px;
    padding: 0 var(--s2);
    border: 0;
    border-radius: calc(var(--radius) - 2px);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--hover);
  }
</style>
