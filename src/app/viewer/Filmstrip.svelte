<script lang="ts">
  // A thin strip of the files ←/→ moves through. Only the visible thumbnails exist, so folders
  // with thousands of files stay fast.
  import { goTo, ui } from "../../lib/app.svelte";
  import { cachedThumb, keepOnly, thumb } from "../../lib/thumbs";

  const ITEM = 60; // width incl. gap
  let strip: HTMLDivElement;
  let width = $state(0);
  let scrollLeft = $state(0);
  let urls = $state<Record<string, string | null>>({});

  const first = $derived(Math.max(0, Math.floor(scrollLeft / ITEM) - 4));
  const last = $derived(Math.min(ui.list.length, Math.ceil((scrollLeft + width) / ITEM) + 4));
  const visible = $derived(ui.list.slice(first, last).map((path, k) => ({ path, i: first + k })));

  // Load thumbnails for what's on screen.
  $effect(() => {
    const paths = new Set(visible.map((v) => v.path));
    keepOnly(paths);
    for (const { path } of visible) {
      if (path in urls) continue;
      const hit = cachedThumb(path);
      if (hit !== undefined) {
        urls[path] = hit;
        continue;
      }
      thumb(path).then((u) => {
        if (paths.has(path)) urls[path] = u;
      });
    }
  });

  // Keep the current file centered.
  $effect(() => {
    const i = ui.index;
    if (!strip || i < 0) return;
    const target = i * ITEM - (width - ITEM) / 2;
    strip.scrollTo({ left: Math.max(0, target), behavior: getComputedStyle(document.documentElement).getPropertyValue("--motion").trim() === "0ms" ? "auto" : "smooth" });
  });

  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const ext = (p: string) => (p.split(".").pop() ?? "").toUpperCase();
</script>

<div class="strip" bind:this={strip} bind:clientWidth={width} onscroll={() => (scrollLeft = strip.scrollLeft)} role="listbox" aria-label="Files" aria-orientation="horizontal">
  <div class="track" style:width="{ui.list.length * ITEM + 8}px">
    {#each visible as v (v.path)}
      <button
        type="button"
        class="item"
        role="option"
        aria-selected={v.i === ui.index}
        title={name(v.path)}
        style:left="{v.i * ITEM + 8}px"
        tabindex="-1"
        onclick={() => goTo(v.i)}
      >
        {#if urls[v.path]}
          <img src={urls[v.path]} alt="" draggable="false" />
        {:else}
          <span class="ext">{ext(v.path)}</span>
        {/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .strip {
    flex: none;
    height: 60px;
    overflow-x: auto;
    overflow-y: hidden;
    background: var(--bg);
    border-top: 1px solid var(--line);
    scrollbar-width: none;
  }
  .track {
    position: relative;
    height: 100%;
  }
  .item {
    position: absolute;
    top: 8px;
    width: 52px;
    height: 42px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    overflow: hidden;
    background: var(--canvas);
    cursor: pointer;
    opacity: 0.72;
    transition: opacity var(--motion) ease-out;
  }
  .item:hover {
    opacity: 1;
  }
  .item[aria-selected="true"] {
    opacity: 1;
    box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px var(--focus);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .ext {
    display: grid;
    place-items: center;
    height: 100%;
    font-size: 10px;
    font-weight: var(--w-medium);
    color: var(--text-dim);
  }
</style>
