<script lang="ts">
  // The timeline: frame thumbnails (video), waveform, what's kept and cut, trim handles,
  // selection and playhead. Drag a handle to trim; drag across to select a part; click to seek;
  // scroll to zoom.
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { commitMedia, fmtTime, keptSegments, liveSplits, media, partAt, silencedAt, snappedSegments } from "./session.svelte";

  let { path, video, seek }: { path: string; video: boolean; seek: (t: number) => void } = $props();

  let canvas: HTMLCanvasElement;
  let width = $state(0);
  const HEIGHT = $derived(video ? 96 : 120);
  const RULER = 18;
  const thumbH = $derived(video ? 48 : 0);
  let peaks = $state<Float32Array | null>(null);
  const frames = new Map<number, HTMLImageElement | null>();
  let frameTick = $state(0);

  const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

  const x2t = (x: number) => media.view.start + (x / width) * (media.view.end - media.view.start);
  const t2x = (t: number) => ((t - media.view.start) / (media.view.end - media.view.start)) * width;

  onMount(() => {
    invoke<number[]>("media_waveform", { path, buckets: 4000 })
      .then((p) => {
        peaks = Float32Array.from(p);
        media.peak = peaks.reduce((m, v) => Math.max(m, Math.abs(v)), 0);
      })
      .catch(() => (peaks = new Float32Array(0)));
  });

  // Thumbnails for the visible stretch, one per ~(thumb width), fetched a few at a time.
  let pending = 0;
  const queue: number[] = [];
  function wantFrame(t: number) {
    if (frames.has(t)) return;
    frames.set(t, null);
    queue.push(t);
    pump();
  }
  function pump() {
    while (pending < 2 && queue.length) {
      const t = queue.shift()!;
      pending++;
      invoke<ArrayBuffer>("media_frame", { path, time: t, height: thumbH * 2 })
        .then((buf) => {
          const img = new Image();
          img.src = URL.createObjectURL(new Blob([buf], { type: "image/jpeg" }));
          img.onload = () => {
            frames.set(t, img);
            frameTick++;
          };
        })
        .catch(() => {})
        .finally(() => {
          pending--;
          pump();
        });
    }
  }

  let frame = 0;
  $effect(() => {
    void [JSON.stringify(media.edit), media.time, media.selection, media.view.start, media.view.end, width, peaks, frameTick, media.mode, media.keyframes.length];
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(draw);
  });

  function draw() {
    if (!canvas || !width || !media.duration) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(HEIGHT * dpr);
    const c = canvas.getContext("2d")!;
    c.setTransform(dpr, 0, 0, dpr, 0, 0);
    c.clearRect(0, 0, width, HEIGHT);
    const text = css("--text"), dim = css("--text-dim"), accent = css("--accent"), line = css("--line"), panel = css("--panel");
    const top = RULER;
    const bodyH = HEIGHT - RULER;

    // Ruler with time labels.
    c.fillStyle = dim;
    c.font = `11px ${css("--font")}`;
    c.textBaseline = "middle";
    const span = media.view.end - media.view.start;
    const step = [0.1, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600].find((s) => (s / span) * width > 70) ?? 3600;
    for (let t = Math.ceil(media.view.start / step) * step; t <= media.view.end; t += step) {
      const x = t2x(t);
      c.fillRect(Math.round(x), RULER - 5, 1, 4);
      c.fillText(fmtTime(t, step < 1), x + 3, RULER / 2 - 1);
    }

    // Track background.
    c.fillStyle = panel;
    c.fillRect(0, top, width, bodyH);

    // Frame thumbnails.
    if (video) {
      const img0 = [...frames.values()].find(Boolean);
      const tw = img0 ? (img0.width / img0.height) * thumbH : thumbH * 1.78;
      for (let x = 0; x < width; x += tw) {
        const t = Math.min(media.duration - 0.05, Math.max(0, x2t(x + tw / 2)));
        const key = Math.round(t * 4) / 4; // quarter-second buckets reuse frames while zooming
        const img = frames.get(key);
        if (img) c.drawImage(img, x, top, tw, thumbH);
        else wantFrame(key);
      }
    }

    // Waveform.
    const waveTop = top + thumbH;
    const waveH = bodyH - thumbH;
    if (peaks && peaks.length) {
      const n = peaks.length / 2;
      c.fillStyle = media.edit.mute ? line : dim;
      const mid = waveTop + waveH / 2;
      for (let x = 0; x < width; x++) {
        if (silencedAt(x2t(x))) {
          c.fillRect(x, mid, 1, 1);
          continue;
        }
        const a = Math.floor((x2t(x) / media.duration) * n);
        const b = Math.max(a + 1, Math.floor((x2t(x + 1) / media.duration) * n));
        let lo = 0, hi = 0;
        for (let i = Math.max(0, a); i < Math.min(n, b); i++) {
          lo = Math.min(lo, peaks[i * 2]);
          hi = Math.max(hi, peaks[i * 2 + 1]);
        }
        const g = Math.pow(10, media.edit.gain / 20);
        const y0 = mid - Math.min(1, hi * g) * (waveH / 2 - 2);
        const y1 = mid - Math.max(-1, lo * g) * (waveH / 2 - 2);
        c.fillRect(x, y0, 1, Math.max(1, y1 - y0));
      }
    }

    // What's kept vs removed.
    const kept = keptSegments();
    let at = 0;
    const shade = (a: number, b: number, hatch: boolean) => {
      const x0 = Math.max(0, t2x(a)), x1 = Math.min(width, t2x(b));
      if (x1 <= x0) return;
      c.fillStyle = "rgba(0,0,0,0.6)";
      c.fillRect(x0, top, x1 - x0, bodyH);
      if (hatch) {
        c.save();
        c.beginPath();
        c.rect(x0, top, x1 - x0, bodyH);
        c.clip();
        c.strokeStyle = "rgba(255,255,255,0.18)";
        c.lineWidth = 1;
        for (let x = x0 - bodyH; x < x1; x += 8) {
          c.beginPath();
          c.moveTo(x, top + bodyH);
          c.lineTo(x + bodyH, top);
          c.stroke();
        }
        c.restore();
      }
    };
    for (const s of kept) {
      if (s.start > at) shade(at, s.start, at >= media.edit.trim.start && s.start <= media.edit.trim.end);
      at = s.end;
    }
    if (at < media.duration) shade(at, media.duration, false);
    if (!kept.length) shade(0, media.duration, true);

    // Where lossless cuts really start (keyframes), when that differs.
    if (media.mode === "lossless" && video) {
      c.fillStyle = accent;
      for (const [i, s] of snappedSegments(kept, media.keyframes).entries()) {
        if (Math.abs(s.start - kept[i].start) > 0.04) {
          const x = t2x(s.start);
          c.globalAlpha = 0.85;
          c.fillRect(Math.round(x), top, 2, bodyH);
          c.globalAlpha = 1;
        }
      }
    }

    // Split marks: a dashed line with a notch on the ruler.
    c.fillStyle = text;
    c.strokeStyle = text;
    c.lineWidth = 1;
    for (const t of liveSplits()) {
      const x = Math.round(t2x(t)) + 0.5;
      if (x < 0 || x > width) continue;
      c.setLineDash([3, 3]);
      c.beginPath();
      c.moveTo(x, top);
      c.lineTo(x, top + bodyH);
      c.stroke();
      c.setLineDash([]);
      c.beginPath();
      c.moveTo(x - 4, top);
      c.lineTo(x + 4, top);
      c.lineTo(x, top + 5);
      c.fill();
    }

    // Selection.
    if (media.selection) {
      const x0 = t2x(media.selection.start), x1 = t2x(media.selection.end);
      c.fillStyle = accent;
      c.globalAlpha = 0.25;
      c.fillRect(x0, top, x1 - x0, bodyH);
      c.globalAlpha = 1;
      c.fillRect(x0, top, 1.5, bodyH);
      c.fillRect(x1 - 1.5, top, 1.5, bodyH);
    }

    // Trim handles: brackets on the kept range's ends.
    for (const [t, dir] of [[media.edit.trim.start, 1], [media.edit.trim.end, -1]] as const) {
      const x = t2x(t);
      c.fillStyle = accent;
      const hx = dir === 1 ? x : x - 10;
      c.beginPath();
      c.roundRect(hx, top, 10, bodyH, dir === 1 ? [4, 0, 0, 4] : [0, 4, 4, 0]);
      c.fill();
      c.fillStyle = css("--on-accent");
      c.fillRect(hx + 4, top + bodyH / 2 - 8, 2, 16);
    }
    c.fillStyle = accent;
    c.fillRect(t2x(media.edit.trim.start), top, t2x(media.edit.trim.end) - t2x(media.edit.trim.start), 2);
    c.fillRect(t2x(media.edit.trim.start), top + bodyH - 2, t2x(media.edit.trim.end) - t2x(media.edit.trim.start), 2);

    // Playhead.
    const px = t2x(media.time);
    c.fillStyle = text;
    c.fillRect(Math.round(px) - 1, 2, 2, HEIGHT - 2);
    c.beginPath();
    c.moveTo(px - 5, 0);
    c.lineTo(px + 5, 0);
    c.lineTo(px, 7);
    c.fill();
  }

  // ---- input ----
  type Drag = { kind: "start" | "end" | "select" | "seek"; x0: number; t0: number };
  let drag: Drag | null = null;

  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0) return;
    const x = e.offsetX;
    canvas.setPointerCapture(e.pointerId);
    const near = (t: number) => Math.abs(t2x(t) - x) < 12;
    if (near(media.edit.trim.start) && x >= t2x(media.edit.trim.start) - 4) drag = { kind: "start", x0: x, t0: media.edit.trim.start };
    else if (near(media.edit.trim.end)) drag = { kind: "end", x0: x, t0: media.edit.trim.end };
    else drag = { kind: "seek", x0: x, t0: x2t(x) };
  }

  function onpointermove(e: PointerEvent) {
    const x = e.offsetX;
    canvas.style.cursor =
      Math.abs(t2x(media.edit.trim.start) - x) < 12 || Math.abs(t2x(media.edit.trim.end) - x) < 12 ? "ew-resize" : "text";
    if (!drag) return;
    const t = Math.max(0, Math.min(media.duration, x2t(x)));
    if (drag.kind === "start") {
      media.edit.trim.start = Math.min(t, media.edit.trim.end - 0.1);
      seek(media.edit.trim.start);
    } else if (drag.kind === "end") {
      media.edit.trim.end = Math.max(t, media.edit.trim.start + 0.1);
      seek(media.edit.trim.end);
    } else if (drag.kind === "seek" && Math.abs(x - drag.x0) > 4) {
      drag.kind = "select";
    }
    if (drag.kind === "select") {
      media.selection = { start: Math.min(drag.t0, t), end: Math.max(drag.t0, t) };
    }
  }

  function onpointerup(e: PointerEvent) {
    if (!drag) return;
    if (drag.kind === "seek") {
      media.selection = null;
      seek(Math.max(0, Math.min(media.duration, x2t(e.offsetX))));
    } else if (drag.kind === "start" || drag.kind === "end") {
      commitMedia();
    } else if (media.selection && media.selection.end - media.selection.start < 0.05) {
      media.selection = null;
    }
    drag = null;
  }

  /** Double-click picks the part around that point: between split marks, or the whole kept part. */
  function ondblclick(e: MouseEvent) {
    media.selection = partAt(Math.max(0, Math.min(media.duration, x2t(e.offsetX))));
  }

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    const span = media.view.end - media.view.start;
    if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
      const d = ((e.shiftKey ? e.deltaY : e.deltaX) / width) * span;
      const s = Math.max(0, Math.min(media.duration - span, media.view.start + d));
      media.view = { start: s, end: s + span };
      return;
    }
    const at = x2t(e.offsetX);
    const next = Math.max(Math.min(1, media.duration), Math.min(media.duration, span * Math.exp(e.deltaY * 0.002)));
    const k = (at - media.view.start) / span;
    let s = at - k * next;
    s = Math.max(0, Math.min(media.duration - next, s));
    media.view = { start: s, end: s + next };
  }
</script>

<div class="timeline" bind:clientWidth={width}>
  <canvas
    bind:this={canvas}
    style:width="{width}px"
    style:height="{HEIGHT}px"
    aria-label="Timeline: drag the handles at either end to trim, drag across a part or double-click it to select it"
    {onpointerdown}
    {onpointermove}
    {onpointerup}
    onpointercancel={() => (drag = null)}
    {ondblclick}
    {onwheel}
  ></canvas>
</div>

<style>
  .timeline {
    position: relative;
    user-select: none;
  }
  canvas {
    display: block;
    touch-action: none;
  }
</style>
