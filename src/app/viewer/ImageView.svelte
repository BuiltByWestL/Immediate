<script lang="ts">
  // Photo viewer: fit to window (never enlarged past 100 %), wheel or pinch zooms around the
  // pointer, drag pans, double-click toggles fit / actual size. The previous photo stays on
  // screen until the next one is decoded, so ←/→ never flashes.
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { ui, viewer } from "../../lib/app.svelte";
  import { decodeWithRust } from "../../lib/pixels";

  /** `rust`: the webview can't decode this format; Immediate's own decoder does. */
  let { path, rust = false, onfail }: { path: string; rust?: boolean; onfail: (msg: string) => void } = $props();

  let box: HTMLDivElement;
  /** What's on screen: an <img> URL, or decoded pixels drawn into a canvas. */
  let shown = $state.raw<{ src?: string; pixels?: ImageData; w: number; h: number } | null>(null);
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
    let cancelled = false;
    if (rust) {
      ui.loading = true;
      decodeWithRust(path)
        .then((pixels) => {
          if (cancelled) return;
          shown = { pixels, w: pixels.width, h: pixels.height };
          ui.facts = { width: pixels.width, height: pixels.height };
          fit();
        })
        .catch((e) => !cancelled && onfail(String(e)))
        .finally(() => !cancelled && (ui.loading = false));
      return () => {
        cancelled = true;
      };
    }
    const src = convertFileSrc(path);
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
  // When the whole picture fits there's nothing to pan, so dragging takes the file out of the
  // window instead (into Explorer, Finder, a chat…).
  let dragOut: { x: number; y: number; id: number } | null = null;

  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0) return;
    if (!canPan && ui.file) {
      dragOut = { x: e.clientX, y: e.clientY, id: e.pointerId };
      return;
    }
    dragging = true;
    last = { x: e.clientX, y: e.clientY };
    box.setPointerCapture(e.pointerId);
  }
  function onpointermove(e: PointerEvent) {
    if (dragOut && ui.file && Math.hypot(e.clientX - dragOut.x, e.clientY - dragOut.y) > 6) {
      dragOut = null;
      invoke("start_drag", { path: ui.file.path }).catch(() => {});
      return;
    }
    if (!dragging) return;
    tx += e.clientX - last.x;
    ty += e.clientY - last.y;
    last = { x: e.clientX, y: e.clientY };
    fitted = false;
    clampPan();
  }
  function onpointerup() {
    dragging = false;
    dragOut = null;
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

  /** Draws decoded pixels into the canvas at full size (the view scales it with CSS). */
  function paint(canvas: HTMLCanvasElement, pixels: ImageData) {
    const draw = (p: ImageData) => {
      canvas.width = p.width;
      canvas.height = p.height;
      canvas.getContext("2d")!.putImageData(p, 0, 0);
    };
    draw(pixels);
    return { update: draw };
  }

  const canPan = $derived(shown ? shown.w * scale > bw + 0.5 || shown.h * scale > bh + 0.5 : false);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="box"
  class:grab={canPan}
  title={canPan ? undefined : "Drag the picture into another app to share it"}
  class:dragging
  bind:this={box}
  {onwheel}
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={onpointerup}
  {ondblclick}
>
  {#if shown?.pixels}
    <canvas
      use:paint={shown.pixels}
      class:pixelated={scale >= 3}
      style:width="{shown.w}px"
      style:height="{shown.h}px"
      style:transform="translate({tx}px, {ty}px) scale({scale})"
    ></canvas>
  {:else if shown?.src}
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
  img,
  canvas {
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
