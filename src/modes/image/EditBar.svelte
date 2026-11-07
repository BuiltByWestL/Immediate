<script lang="ts">
  // Toolbar while editing a photo: Cancel · Undo / Redo / Compare · Save (with Save As / Replace).
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Redo from "@lucide/svelte/icons/redo-2";
  import Undo from "@lucide/svelte/icons/undo-2";
  import SplitSquare from "@lucide/svelte/icons/square-split-horizontal";
  import { MOD } from "../../lib/shortcuts";
  import { cancelEdit, save } from "./actions";
  import { editor, redo, undo } from "./editor.svelte";

  let menuOpen = $state(false);
  const k = MOD === "⌘" ? "⌘" : "Ctrl+";

  function pick(t: "as" | "replace") {
    menuOpen = false;
    save(t);
  }
</script>

<svelte:window onclick={() => (menuOpen = false)} />

<div class="side">
  <button type="button" class="btn" onclick={cancelEdit} title="Esc">Cancel</button>
</div>

<div class="center">
  <button type="button" class="icon-btn" title="Undo ({k}Z)" aria-label="Undo" disabled={!editor.past.length} onclick={undo}>
    <Undo size={17} strokeWidth={1.5} />
  </button>
  <button type="button" class="icon-btn" title="Redo ({k}Shift+Z)" aria-label="Redo" disabled={!editor.future.length} onclick={redo}>
    <Redo size={17} strokeWidth={1.5} />
  </button>
  <button
    type="button"
    class="icon-btn"
    title="Hold to compare with the original (\)"
    aria-label="Compare with original"
    aria-pressed={editor.comparing}
    onpointerdown={() => (editor.comparing = true)}
    onpointerup={() => (editor.comparing = false)}
    onpointerleave={() => (editor.comparing = false)}
  >
    <SplitSquare size={17} strokeWidth={1.5} />
  </button>
</div>

<div class="side end">
  <div class="split">
    <button type="button" class="btn primary main" disabled={editor.saving || !editor.ready} onclick={() => save("copy")} title="Save a copy next to the original ({k}S)">
      {editor.saving ? "Saving…" : "Save"}
    </button>
    <button
      type="button"
      class="btn primary more"
      aria-label="More ways to save"
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      disabled={editor.saving || !editor.ready}
      onclick={(e) => {
        e.stopPropagation();
        menuOpen = !menuOpen;
      }}
    >
      <ChevronDown size={16} strokeWidth={1.75} />
    </button>
    {#if menuOpen}
      <div class="menu" role="menu">
        <button type="button" role="menuitem" onclick={() => pick("as")}>
          <span>Save As…</span><span class="dim">{k}Shift+S</span>
        </button>
        <button type="button" role="menuitem" onclick={() => pick("replace")}>
          <span>Replace original…</span>
        </button>
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
    gap: var(--s1);
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
