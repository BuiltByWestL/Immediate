<script lang="ts">
  // The crop over the video: drag inside to move it, drag a corner to resize it (keeping the
  // chosen shape, if any). Outside the frame the picture is dimmed, and clicks there still reach
  // the video. Coordinates are fractions of the shown (turned) picture.
  import { commitMedia, media, type Range2D } from "./session.svelte";

  let { width, height }: { width: number; height: number } = $props();

  type Grip = "move" | "nw" | "ne" | "sw" | "se";
  let drag: { grip: Grip; x: number; y: number; start: Range2D } | null = null;
  const MIN = 0.05;

  function down(e: PointerEvent, grip: Grip) {
    if (e.button !== 0 || !media.edit.crop) return;
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { grip, x: e.clientX, y: e.clientY, start: { ...media.edit.crop } };
  }

  function move(e: PointerEvent) {
    if (!drag) return;
    const dx = (e.clientX - drag.x) / width;
    const dy = (e.clientY - drag.y) / height;
    const s = drag.start;
    if (drag.grip === "move") {
      media.edit.crop = { ...s, x: clamp(s.x + dx, 0, 1 - s.w), y: clamp(s.y + dy, 0, 1 - s.h) };
      return;
    }
    // The corner across from the one being dragged stays put.
    const west = drag.grip.includes("w");
    const north = drag.grip.includes("n");
    const ax = west ? s.x + s.w : s.x;
    const ay = north ? s.y + s.h : s.y;
    let w = clamp(west ? s.w - dx : s.w + dx, MIN, west ? ax : 1 - ax);
    let h = clamp(north ? s.h - dy : s.h + dy, MIN, north ? ay : 1 - ay);
    const r = media.cropAspect;
    if (r) {
      // Keep the shape (r is width / height in pixels): follow the larger change, then fit.
      const hFromW = (w * width) / r / height;
      if (hFromW <= (north ? ay : 1 - ay)) h = Math.max(hFromW, MIN);
      else {
        h = north ? ay : 1 - ay;
        w = (h * height * r) / width;
      }
    }
    media.edit.crop = { x: west ? ax - w : ax, y: north ? ay - h : ay, w, h };
  }

  function up() {
    if (!drag) return;
    drag = null;
    commitMedia();
  }

  const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
  const c = $derived(media.edit.crop!);
</script>

<div class="frame" style:width="{width}px" style:height="{height}px">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="crop"
    style:left="{c.x * 100}%"
    style:top="{c.y * 100}%"
    style:width="{c.w * 100}%"
    style:height="{c.h * 100}%"
    onpointerdown={(e) => down(e, "move")}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
  >
    <span class="third v1"></span><span class="third v2"></span><span class="third h1"></span><span class="third h2"></span>
    {#each ["nw", "ne", "sw", "se"] as g (g)}
      <span class="grip {g}" onpointerdown={(e) => down(e, g as Grip)} onpointermove={move} onpointerup={up} onpointercancel={up}></span>
    {/each}
  </div>
</div>

<style>
  .frame {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    overflow: hidden;
    pointer-events: none;
  }
  .crop {
    position: absolute;
    border: 1px solid rgb(255 255 255 / 0.9);
    box-shadow: 0 0 0 9999px rgb(0 0 0 / 0.55);
    cursor: move;
    pointer-events: auto;
    touch-action: none;
  }
  .third {
    position: absolute;
    background: rgb(255 255 255 / 0.25);
    pointer-events: none;
  }
  .v1,
  .v2 {
    top: 0;
    bottom: 0;
    width: 1px;
  }
  .v1 {
    left: 33.33%;
  }
  .v2 {
    left: 66.66%;
  }
  .h1,
  .h2 {
    left: 0;
    right: 0;
    height: 1px;
  }
  .h1 {
    top: 33.33%;
  }
  .h2 {
    top: 66.66%;
  }
  .grip {
    position: absolute;
    width: 14px;
    height: 14px;
    border: 2px solid #fff;
    background: rgb(0 0 0 / 0.3);
  }
  .nw {
    left: -7px;
    top: -7px;
    cursor: nwse-resize;
  }
  .ne {
    right: -7px;
    top: -7px;
    cursor: nesw-resize;
  }
  .sw {
    left: -7px;
    bottom: -7px;
    cursor: nesw-resize;
  }
  .se {
    right: -7px;
    bottom: -7px;
    cursor: nwse-resize;
  }
</style>
