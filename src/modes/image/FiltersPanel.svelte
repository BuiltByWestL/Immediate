<script lang="ts">
  // Filter previews are real renders of this photo through each filter.
  import { onMount } from "svelte";
  import { FILTERS } from "./edits";
  import { change, commit, editor, session } from "./editor.svelte";
  import Slider from "./Slider.svelte";

  const THUMB = 160;
  let thumbs = $state<Record<string, string>>({});

  onMount(() => {
    const p = session.pipeline;
    if (!p) return;
    const base = $state.snapshot(editor.edits);
    const scale = Math.min(1, THUMB / Math.max(p.src.w, p.src.h));
    const make = (id: string | null) => {
      const edits = { ...base, filter: { id, amount: 100 } };
      const { data, w, h } = p.renderFull(edits, scale);
      const c = document.createElement("canvas");
      c.width = w;
      c.height = h;
      c.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(data.buffer as ArrayBuffer), w, h), 0, 0);
      return c.toDataURL("image/jpeg", 0.85);
    };
    // Render a few per frame so the panel opens immediately.
    const queue: (string | null)[] = [null, ...FILTERS.map((f) => f.id)];
    let raf = 0;
    const step = () => {
      for (let i = 0; i < 3 && queue.length; i++) {
        const id = queue.shift()!;
        thumbs = { ...thumbs, [id ?? "none"]: make(id) };
      }
      if (queue.length) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => {
      cancelAnimationFrame(raf);
    };
  });

  const items = [{ id: null as string | null, name: "Original" }, ...FILTERS.map((f) => ({ id: f.id as string | null, name: f.name }))];
</script>

<div class="grid" role="radiogroup" aria-label="Filters">
  {#each items as f (f.id ?? "none")}
    <button
      type="button"
      class="filter"
      role="radio"
      aria-checked={editor.edits.filter.id === f.id}
      onclick={() => change((e) => (e.filter = { id: f.id, amount: 100 }))}
    >
      <span class="thumb">
        {#if thumbs[f.id ?? "none"]}<img src={thumbs[f.id ?? "none"]} alt="" />{/if}
      </span>
      <span class="name">{f.name}</span>
    </button>
  {/each}
</div>

{#if editor.edits.filter.id}
  <div class="amount">
    <Slider label="Amount" min={0} max={100} bind:value={editor.edits.filter.amount} oncommit={commit} />
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--s3);
    padding-top: var(--s1);
  }
  .filter {
    display: grid;
    gap: var(--s1);
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .thumb {
    display: grid;
    place-items: center;
    aspect-ratio: 4 / 3;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--canvas);
    box-shadow: 0 0 0 1px var(--line);
  }
  .filter[aria-checked="true"] .thumb {
    box-shadow: 0 0 0 2px var(--focus);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .name {
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
  }
  .amount {
    margin-top: var(--s4);
    padding-top: var(--s3);
    border-top: 1px solid var(--line);
  }
</style>
