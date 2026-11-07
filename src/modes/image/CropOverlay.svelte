<script lang="ts">
  // The crop frame over the whole (straightened) photo: drag inside to move, drag a corner or
  // edge to resize. It never leaves the photo, and keeps the chosen aspect ratio.
  import { commit, editor } from "./editor.svelte";
  import { cropInside, effectiveCrop, rotatedSize, type Rect, type Size } from "./geometry";

  // `area`: the part of rotated space on screen (its top-left is at tx, ty).
  let { scale, tx, ty, src, area }: { scale: number; tx: number; ty: number; src: Size; area: Rect } = $props();

  const g = $derived(editor.edits.geometry);
  const rot = $derived(rotatedSize(src, g));
  const crop = $derived(effectiveCrop(g, src));
  // Crop in CSS pixels within the stage.
  const r = $derived({
    x: tx + (crop.x - area.x) * rot.w * scale,
    y: ty + (crop.y - area.y) * rot.h * scale,
    w: crop.w * rot.w * scale,
    h: crop.h * rot.h * scale,
  });

  type Handle = "move" | "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";
  let drag: { handle: Handle; start: Rect; px: number; py: number } | null = $state(null);
  const MIN_PX = 24;

  function onpointerdown(e: PointerEvent, handle: Handle) {
    if (e.button !== 0) return;
    e.stopPropagation();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    drag = { handle, start: { ...crop }, px: e.clientX, py: e.clientY };
  }

  function onpointermove(e: PointerEvent) {
    if (!drag) return;
    const dx = (e.clientX - drag.px) / (rot.w * scale);
    const dy = (e.clientY - drag.py) / (rot.h * scale);
    const s = drag.start;
    let { x, y, w, h } = s;
    const hd = drag.handle;
    if (hd === "move") {
      x = Math.min(1 - w, Math.max(0, x + dx));
      y = Math.min(1 - h, Math.max(0, y + dy));
    } else {
      const minW = MIN_PX / (rot.w * scale);
      const minH = MIN_PX / (rot.h * scale);
      let left = s.x, top = s.y, right = s.x + s.w, bottom = s.y + s.h;
      if (hd.includes("w")) left = Math.min(right - minW, Math.max(0, left + dx));
      if (hd.includes("e")) right = Math.max(left + minW, Math.min(1, right + dx));
      if (hd.includes("n")) top = Math.min(bottom - minH, Math.max(0, top + dy));
      if (hd.includes("s")) bottom = Math.max(top + minH, Math.min(1, bottom + dy));
      x = left; y = top; w = right - left; h = bottom - top;
      if (g.aspect) {
        // Keep the ratio: the dragged dimension leads; edges drive the other one from the center.
        const ratio = g.aspect * (rot.h / rot.w); // fraction-space w/h
        const horizontal = hd === "e" || hd === "w" || (hd.length === 2 && Math.abs(dx) * rot.w >= Math.abs(dy) * rot.h);
        if (horizontal) {
          const nh = w / ratio;
          if (hd === "e" || hd === "w") y = s.y + (s.h - nh) / 2;
          else if (hd.includes("n")) y = bottom - nh;
          h = nh;
        } else {
          const nw = h * ratio;
          if (hd === "n" || hd === "s") x = s.x + (s.w - nw) / 2;
          else if (hd.includes("w")) x = right - nw;
          w = nw;
        }
        if (x < 0 || y < 0 || x + w > 1 || y + h > 1) return;
      }
    }
    const next = { x, y, w, h };
    // Stay on the photo: back off along the drag until the frame fits.
    if (cropInside(next, g, src)) {
      editor.edits.geometry.crop = next;
    } else {
      let lo = 0, hi = 1;
      const lerp = (t: number): Rect => ({ x: s.x + (x - s.x) * t, y: s.y + (y - s.y) * t, w: s.w + (w - s.w) * t, h: s.h + (h - s.h) * t });
      for (let i = 0; i < 16; i++) {
        const mid = (lo + hi) / 2;
        if (cropInside(lerp(mid), g, src)) lo = mid;
        else hi = mid;
      }
      editor.edits.geometry.crop = lerp(lo);
    }
  }

  function onpointerup() {
    if (!drag) return;
    drag = null;
    commit();
  }

  const HANDLES: Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
  const pos = (h: Handle) => ({
    left: h.includes("w") ? 0 : h.includes("e") ? 100 : 50,
    top: h.includes("n") ? 0 : h.includes("s") ? 100 : 50,
  });
</script>

<div class="overlay">
  <!-- Dim everything outside the frame. -->
  <svg class="shade" aria-hidden="true">
    <path fill-rule="evenodd" d="M0 0H10000V10000H0Z M{r.x} {r.y}h{r.w}v{r.h}h{-r.w}Z" />
  </svg>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="frame"
    class:dragging={!!drag}
    style:left="{r.x}px"
    style:top="{r.y}px"
    style:width="{r.w}px"
    style:height="{r.h}px"
    onpointerdown={(e) => onpointerdown(e, "move")}
    {onpointermove}
    {onpointerup}
    onpointercancel={onpointerup}
  >
    <div class="thirds" aria-hidden="true"><span></span><span></span><span></span><span></span></div>
    {#each HANDLES as hd (hd)}
      <span
        class="handle {hd}"
        style:left="{pos(hd).left}%"
        style:top="{pos(hd).top}%"
        onpointerdown={(e) => onpointerdown(e, hd)}
        {onpointermove}
        {onpointerup}
        onpointercancel={onpointerup}
        aria-hidden="true"
      ></span>
    {/each}
  </div>
</div>

<style>
  .overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .shade {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .shade path {
    fill: rgb(0 0 0 / 0.55);
  }
  .frame {
    position: absolute;
    pointer-events: auto;
    cursor: move;
    box-shadow: 0 0 0 1px rgb(255 255 255 / 0.9), 0 0 0 2px rgb(0 0 0 / 0.25);
    touch-action: none;
  }
  .thirds {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity var(--motion) ease-out;
  }
  .dragging .thirds {
    opacity: 1;
  }
  .thirds span {
    position: absolute;
    background: rgb(255 255 255 / 0.5);
  }
  .thirds span:nth-child(1) { left: 33.333%; top: 0; bottom: 0; width: 1px; }
  .thirds span:nth-child(2) { left: 66.666%; top: 0; bottom: 0; width: 1px; }
  .thirds span:nth-child(3) { top: 33.333%; left: 0; right: 0; height: 1px; }
  .thirds span:nth-child(4) { top: 66.666%; left: 0; right: 0; height: 1px; }
  .handle {
    position: absolute;
    width: 22px;
    height: 22px;
    margin: -11px 0 0 -11px;
  }
  .handle::after {
    content: "";
    position: absolute;
    left: 7px;
    top: 7px;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: #fff;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 0.3);
  }
  .n, .s { cursor: ns-resize; }
  .e, .w { cursor: ew-resize; }
  .nw, .se { cursor: nwse-resize; }
  .ne, .sw { cursor: nesw-resize; }
</style>
