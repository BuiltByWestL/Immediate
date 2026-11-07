<script lang="ts">
  // Photo viewer: fit to window (never enlarged past 100 %), wheel or pinch zooms around the
  // pointer, drag pans, double-click toggles fit / actual size. The previous photo stays on
  // screen until the next one is decoded, so ←/→ never flashes.
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { ui, viewer } from "../../lib/app.svelte";

  let { path, onfail }: { path: string; onfail: (msg: string) => void } = $props();

  let box: HTMLDivElement;
  let shown = $state<{ src: string; w: number; h: number } | null>(null);
  let scale = $state(1);
  let tx = $state(0);
  let ty = $state(0);
  let fitted = $state(true);
  let dragging = $state(false);
  let bw = $state(0);
  let bh = $state(0);

  const MAX = 32;
  const fitScale = () => (shown ? Math.min(1, box.clientWidth / shown.w, box.clientHeight / shown.h) : 1);
  const minScale = () => Math.min(fitScale(), 0.05) || 0.05;

  function fit() {
    if (!shown) return;
    scale = fitScale();
    tx = (box.clientWidth - shown.w * scale) / 2;
    ty = (box.clientHeight - shown.h * scale) / 2;
    fitted = true;
  }

  /** Zooms to `next`, keeping the image point under (cx, cy) in place. */
  function zoomTo(next: number, cx = box.clientWidth / 2, cy = box.clientHeight / 2) {
    if (!shown) return;
    next = Math.min(MAX, Math.max(minScale(), next));
    tx = cx - ((cx - tx) * next) / scale;
    ty = cy - ((cy - ty) * next) / scale;
    scale = next;
    fitted = false;
    clampPan();
  }

  /** Keeps at least part of the image on screen; centers an axis that fits. */
  function clampPan() {
    if (!shown) return;
    const w = shown.w * scale;
    const h = shown.h * scale;
    const bw = box.clientWidth;
    const bh = box.clientHeight;
    tx = w <= bw ? (bw - w) / 2 : Math.min(0, Math.max(bw - w, tx));
    ty = h <= bh ? (bh - h) / 2 : Math.min(0, Math.max(bh - h, ty));
  }

  $effect(() => {
    const src = convertFileSrc(path);
    let cancelled = false;
    const img = new Image();
    img.decoding = "async";
    img.src = src;
    img
      .decode()
      .then(() => {
        if (cancelled) return;
        shown = { src, w: img.naturalWidth, h: img.naturalHeight };
        ui.facts = { width: img.naturalWidth, height: img.naturalHeight };
        fit();
      })
      .catch(() => {
        if (!cancelled) onfail("This picture couldn’t be read. It may be damaged, or saved in a variant Immediate can’t show yet.");
      });
    return () => {
      cancelled = true;
    };
  });

  // Report zoom to the toolbar.
  $effect(() => {
    viewer.zoom = shown ? scale : null;
  });

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    const r = box.getBoundingClientRect();
    // Pinch arrives as ctrl+wheel with small deltas; mouse wheels as larger steps.
    const k = e.ctrlKey ? 0.01 : 0.0025;
    zoomTo(scale * Math.exp(-e.deltaY * k), e.clientX - r.left, e.clientY - r.top);
  }

  let last = { x: 0, y: 0 };
  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0) return;
    dragging = true;
    last = { x: e.clientX, y: e.clientY };
    box.setPointerCapture(e.pointerId);
  }
  function onpointermove(e: PointerEvent) {
    if (!dragging) return;
    tx += e.clientX - last.x;
    ty += e.clientY - last.y;
    last = { x: e.clientX, y: e.clientY };
    fitted = false;
    clampPan();
  }
  function onpointerup() {
    dragging = false;
  }
  function ondblclick(e: MouseEvent) {
    const r = box.getBoundingClientRect();
    if (fitted && fitScale() < 1) zoomTo(1, e.clientX - r.left, e.clientY - r.top);
    else fit();
  }

  onMount(() => {
    const ro = new ResizeObserver(() => {
      bw = box.clientWidth;
      bh = box.clientHeight;
      if (fitted) fit();
      else clampPan();
    });
    ro.observe(box);
    viewer.command = (cmd) => {
      if (cmd === "fit") fit();
      else if (cmd === "actual") zoomTo(1);
      else if (cmd === "in") zoomTo(scale * 1.25);
      else if (cmd === "out") zoomTo(scale / 1.25);
      else return false;
      return true;
    };
    return () => {
      ro.disconnect();
      viewer.command = null;
      viewer.zoom = null;
    };
  });

  const canPan = $derived(shown ? shown.w * scale > bw + 0.5 || shown.h * scale > bh + 0.5 : false);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="box"
  class:grab={canPan}
  class:dragging
  bind:this={box}
  {onwheel}
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={onpointerup}
  {ondblclick}
>
  {#if shown}
    <img
      src={shown.src}
      alt={ui.file?.name ?? ""}
      draggable="false"
      class:pixelated={scale >= 3}
      style:width="{shown.w}px"
      style:height="{shown.h}px"
      style:transform="translate({tx}px, {ty}px) scale({scale})"
    />
  {/if}
</div>

<style>
  .box {
    position: absolute;
    inset: 0;
    overflow: hidden;
    touch-action: none;
  }
  .grab {
    cursor: grab;
  }
  .dragging {
    cursor: grabbing;
  }
  img {
    position: absolute;
    left: 0;
    top: 0;
    transform-origin: 0 0;
    max-width: none;
    user-select: none;
    -webkit-user-drag: none;
  }
  .pixelated {
    image-rendering: pixelated;
  }
</style>
