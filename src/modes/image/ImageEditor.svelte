<script lang="ts">
  // The photo editor: a WebGL canvas showing the edited photo live, and the inspector panel.
  import { invoke } from "@tauri-apps/api/core";
  import { needsDecoding } from "../../lib/formats";
  import { loadBitmap } from "../../lib/pixels";
  import { onMount } from "svelte";
  import { viewer } from "../../lib/app.svelte";
  import type { OpenedFile } from "../../lib/types";
  import { editor, session } from "./editor.svelte";
  import CropOverlay from "./CropOverlay.svelte";
  import { cropViewBox, rotatedSize } from "./geometry";
  import Inspector from "./Inspector.svelte";
  import MarkupLayer from "./MarkupLayer.svelte";
  import { outputSize, Pipeline } from "./pipeline";
  import type { ImageInfo } from "./save";

  let { file }: { file: OpenedFile } = $props();

  let box: HTMLDivElement;
  let canvas = $state.raw() as HTMLCanvasElement;
  let pipeline = $state.raw<Pipeline | null>(null);
  let bw = $state(0);
  let bh = $state(0);
  let dpr = window.devicePixelRatio || 1;

  // View: CSS pixels per output pixel, and the output's top-left in the box.
  let scale = $state(1);
  let tx = $state(0);
  let ty = $state(0);
  let fitted = $state(true);
  let out = $state({ w: 1, h: 1 });

  const fitScale = () => Math.min(1, (bw - 48) / out.w, (bh - 48) / out.h);

  function fit() {
    scale = Math.max(0.01, fitScale());
    tx = (bw - out.w * scale) / 2;
    ty = (bh - out.h * scale) / 2;
    fitted = true;
  }

  function zoomTo(next: number, cx = bw / 2, cy = bh / 2) {
    next = Math.min(16, Math.max(Math.min(fitScale(), 0.05), next));
    tx = cx - ((cx - tx) * next) / scale;
    ty = cy - ((cy - ty) * next) / scale;
    scale = next;
    fitted = false;
    clampPan();
  }

  function clampPan() {
    const w = out.w * scale;
    const h = out.h * scale;
    tx = w <= bw ? (bw - w) / 2 : Math.min(0, Math.max(bw - w, tx));
    ty = h <= bh ? (bh - h) / 2 : Math.min(0, Math.max(bh - h, ty));
  }

  // The crop tool shows the whole photo; elsewhere the cropped result.
  const cropping = $derived(editor.tab === "crop");

  // Output size follows crop/rotation; refit when it changes.
  $effect(() => {
    if (!editor.ready || !pipeline) return;
    const g = editor.edits.geometry;
    const rot = rotatedSize(pipeline.src, g);
    const area = cropViewBox(g, pipeline.src);
    const o = cropping ? { w: Math.round(rot.w * area.w), h: Math.round(rot.h * area.h) } : outputSize(pipeline.src, g);
    if (o.w !== out.w || o.h !== out.h) {
      out = o;
      fit();
    }
  });

  // Draw whenever anything that affects the picture changes, at most once per frame.
  let frame = 0;
  $effect(() => {
    const key = JSON.stringify(editor.edits) + editor.comparing + scale + tx + ty + bw + bh + editor.ready + cropping;
    void key;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(draw);
  });

  function draw() {
    if (!pipeline || !editor.ready) return;
    const w = Math.round(bw * dpr);
    const h = Math.round(bh * dpr);
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    const area = cropping ? cropViewBox(editor.edits.geometry, pipeline.src) : undefined;
    pipeline.render(editor.edits, { scale: scale * dpr, x: tx * dpr, y: ty * dpr }, editor.comparing, area);
  }

  async function load() {
    editor.ready = false;
    editor.error = "";
    try {
      const info = await invoke<ImageInfo>("image_info", { path: file.path });
      session.info = info;
      // Pixels stay in the photo's own color space (sRGB or Display P3, drawn as such), so
      // nothing shifts on save. Other profiles are converted to sRGB.
      const opts: ImageBitmapOptions = {
        colorSpaceConversion: info.colorSpace === "other" ? "default" : "none",
      };
      const rust = needsDecoding(file);
      let bmp = await loadBitmap(file.path, rust, opts);
      pipeline = new Pipeline(canvas, info.colorSpace === "display-p3" ? "display-p3" : "srgb");
      if (bmp.width > pipeline.maxSize || bmp.height > pipeline.maxSize) {
        const k = pipeline.maxSize / Math.max(bmp.width, bmp.height);
        const full = bmp;
        bmp = await createImageBitmap(full, { resizeWidth: Math.floor(full.width * k), resizeHeight: Math.floor(full.height * k), resizeQuality: "high" });
        full.close();
      }
      pipeline.setSource(bmp, { w: bmp.width, h: bmp.height });
      bmp.close();
      session.pipeline = pipeline;
      out = outputSize(pipeline.src, editor.edits.geometry);
      fit();
      editor.ready = true;
    } catch (e) {
      editor.error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(() => {
    const ro = new ResizeObserver(() => {
      bw = box.clientWidth;
      bh = box.clientHeight;
      dpr = window.devicePixelRatio || 1;
      if (fitted) fit();
      else clampPan();
    });
    ro.observe(box);
    load();
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
      cancelAnimationFrame(frame);
      viewer.command = null;
      viewer.zoom = null;
      session.pipeline = null;
      pipeline?.dispose();
      pipeline = null;
    };
  });

  $effect(() => {
    viewer.zoom = editor.ready ? scale : null;
  });

  // Pan by dragging, zoom with the wheel.
  let drag: { x: number; y: number } | null = null;
  function onwheel(e: WheelEvent) {
    e.preventDefault();
    const r = box.getBoundingClientRect();
    zoomTo(scale * Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0025)), e.clientX - r.left, e.clientY - r.top);
  }
  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0) return;
    drag = { x: e.clientX, y: e.clientY };
    box.setPointerCapture(e.pointerId);
  }
  function onpointermove(e: PointerEvent) {
    if (!drag) return;
    tx += e.clientX - drag.x;
    ty += e.clientY - drag.y;
    drag = { x: e.clientX, y: e.clientY };
    fitted = false;
    clampPan();
  }
  function onpointerup() {
    drag = null;
  }
</script>

<div class="editor">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="stage"
    bind:this={box}
    {onwheel}
    {onpointerdown}
    {onpointermove}
    {onpointerup}
    onpointercancel={onpointerup}
    ondblclick={() => (fitted ? zoomTo(1) : fit())}
  >
    <canvas bind:this={canvas} style:width="{bw}px" style:height="{bh}px"></canvas>
    {#if !cropping && editor.ready && pipeline}
      <MarkupLayer {scale} {tx} {ty} {bw} {bh} src={pipeline.src} glCanvas={canvas} active={editor.tab === "markup"} />
    {/if}
    {#if cropping && editor.ready && pipeline}
      <CropOverlay {scale} {tx} {ty} src={pipeline.src} area={cropViewBox(editor.edits.geometry, pipeline.src)} />
    {/if}
    {#if editor.comparing}
      <span class="badge">Original</span>
    {/if}
    {#if editor.error}
      <div class="error"><p>{editor.error}</p></div>
    {/if}
  </div>
  <Inspector />
</div>

<style>
  .editor {
    flex: 1;
    display: flex;
    min-width: 0;
  }
  .stage {
    position: relative;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    background: var(--canvas);
    touch-action: none;
  }
  canvas {
    position: absolute;
    inset: 0;
    display: block;
  }
  .badge {
    position: absolute;
    top: var(--s3);
    left: 50%;
    transform: translateX(-50%);
    padding: 2px var(--s2);
    border-radius: var(--radius);
    background: var(--raised);
    border: 1px solid var(--line);
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
  }
  .error {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: var(--s6);
    color: var(--danger);
    text-align: center;
  }
</style>
