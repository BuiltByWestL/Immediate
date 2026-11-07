<script lang="ts">
  // A modal sheet on <dialog>: focus is trapped, Esc closes, the page behind is inert.
  import X from "@lucide/svelte/icons/x";
  import { onMount, type Snippet } from "svelte";

  let {
    title,
    onclose,
    width = 560,
    flush = false,
    children,
  }: { title: string; onclose: () => void; width?: number; flush?: boolean; children: Snippet } = $props();

  let dlg: HTMLDialogElement;
  onMount(() => {
    dlg.showModal();
    // Start focus on the sheet itself rather than its first button; Tab moves in from there.
    dlg.focus();
  });
</script>

<dialog
  class="sheet"
  bind:this={dlg}
  tabindex="-1"
  aria-label={title}
  style:width="{width}px"
  onclose={onclose}
  onclick={(e) => e.target === dlg && dlg.close()}
>
  <header>
    <h2>{title}</h2>
    <button type="button" class="icon-btn" aria-label="Close" onclick={() => dlg.close()}><X size={16} strokeWidth={1.5} /></button>
  </header>
  <div class="body" class:flush>
    {@render children()}
  </div>
</dialog>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--s3) var(--s3) var(--s3) var(--s5);
    border-bottom: 1px solid var(--line);
  }
  h2 {
    font-size: var(--text-md);
    font-weight: var(--w-medium);
  }
  .body {
    padding: var(--s5);
    overflow-y: auto;
    max-height: calc(100vh - 48px - 53px);
  }
  .body.flush {
    padding: 0;
    overflow: hidden;
  }
</style>
