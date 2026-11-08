<script lang="ts">
  // Draws the markup over the live photo and, on the Markup tab, takes pen/mouse input.
  import { change, editor } from "./editor.svelte";
  import { mapper, type Size } from "./geometry";
  import { hitTest, paintMarkup, SIZES, type Pt, type Shape } from "./markup";

  let {
    scale,
    tx,
    ty,
    bw,
    bh,
    src,
    glCanvas,
    active,
  }: { scale: number; tx: number; ty: number; bw: number; bh: number; src: Size; glCanvas: HTMLCanvasElement; active: boolean } = $props();

  let canvas: HTMLCanvasElement;
  let draft = $state<Shape | null>(null);
  let typing = $state<{ x: number; y: number; at: Pt } | null>(null);
  let text = $state("");
  let input = $state<HTMLTextAreaElement | null>(null);

  const dpr = () => window.devicePixelRatio || 1;
  const m = $derived(mapper(editor.edits.geometry, src));
  const sizePx = $derived(SIZES[editor.size] * Math.max(src.w, src.h));

  /** Screen (stage CSS px) → source px. */
  const toSource = (x: number, y: number): Pt => m.toSource((x - tx) / scale, (y - ty) / scale);
  /** Source px → screen CSS px. */
  const toScreen = (p: Pt): [number, number] => {
    const [ox, oy] = m.toOutput(p[0], p[1]);
    return [tx + ox * scale, ty + oy * scale];
  };

  // Repaint when shapes, view or the photo change (the photo matters for redaction).
  let frame = 0;
  $effect(() => {
    void [JSON.stringify(editor.edits), draft && JSON.stringify(draft), scale, tx, ty, bw, bh, editor.comparing];
    cancelAnimationFrame(frame);
    // After the photo itself has been drawn this frame.
    frame = requestAnimationFrame(() => requestAnimationFrame(paint));
  });

  function paint() {
    if (!canvas) return;
    const d = dpr();
    const w = Math.round(bw * d);
    const h = Math.round(bh * d);
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    const ctx = canvas.getContext("2d")!;
    ctx.clearRect(0, 0, w, h);
    if (editor.comparing) return;
    const shapes = draft ? [...editor.edits.markup, draft] : editor.edits.markup;
    if (!shapes.length) return;
    // Clip to the photo so nothing spills onto the background.
    ctx.save();
    ctx.beginPath();
    ctx.rect(tx * d, ty * d, m.out.w * scale * d, m.out.h * scale * d);
    ctx.clip();
    paintMarkup(ctx, shapes, (p) => {
      const [x, y] = toScreen(p);
      return [x * d, y * d];
    }, scale * d, glCanvas);
    ctx.restore();
  }

  function base(kind: Shape["kind"], pts: Pt[]): Shape {
    // Numbered steps have no Size control; they always use the medium size.
    const size = kind === "text" ? sizePx * 5 : kind === "number" ? SIZES.m * Math.max(src.w, src.h) : sizePx;
    return { kind, pts, color: editor.color, size };
  }

  /** Four corners of the screen-space box a..b, in source px (so boxes stay boxes on screen). */
  function box(a: [number, number], b: [number, number]): Pt[] {
    return [toSource(a[0], a[1]), toSource(b[0], a[1]), toSource(b[0], b[1]), toSource(a[0], b[1])];
  }

  let start: [number, number] | null = null;

  function local(e: PointerEvent): [number, number] {
    const r = canvas.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  }

  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0 || !active) return;
    e.stopPropagation();
    if (typing) commitText();
    const p = local(e);
    const tool = editor.tool;
    if (tool === "eraser") {
      const i = hitTest(editor.edits.markup, toSource(...p), 8 / scale);
      if (i >= 0) change((ed) => ed.markup.splice(i, 1));
      return;
    }
    if (tool === "text") {
      typing = { x: p[0], y: p[1], at: toSource(...p) };
      text = "";
      queueMicrotask(() => input?.focus());
      return;
    }
    if (tool === "number") {
      const n = editor.edits.markup.filter((s) => s.kind === "number").length + 1;
      change((ed) => ed.markup.push({ ...base("number", [toSource(...p)]), n }));
      return;
    }
    canvas.setPointerCapture(e.pointerId);
    start = p;
    draft = tool === "pen" || tool === "highlighter" ? base(tool, [toSource(...p)]) : base(tool, tool === "line" || tool === "arrow" ? [toSource(...p), toSource(...p)] : box(p, p));
  }

  function onpointermove(e: PointerEvent) {
    if (!draft || !start) return;
    const p = local(e);
    if (draft.kind === "pen" || draft.kind === "highlighter") {
      const last = draft.pts[draft.pts.length - 1];
      const next = toSource(...p);
      // Skip points closer than ~1.5 screen px.
      if (Math.hypot(next[0] - last[0], next[1] - last[1]) * scale > 1.5) draft.pts.push(next);
    } else if (draft.kind === "line" || draft.kind === "arrow") {
      let q = p;
      // Shift snaps to 45° steps.
      if (e.shiftKey) {
        const ang = Math.round(Math.atan2(p[1] - start[1], p[0] - start[0]) / (Math.PI / 4)) * (Math.PI / 4);
        const len = Math.hypot(p[0] - start[0], p[1] - start[1]);
        q = [start[0] + len * Math.cos(ang), start[1] + len * Math.sin(ang)];
      }
      draft.pts = [toSource(...start), toSource(...q)];
    } else {
      let q = p;
      // Shift makes squares and circles.
      if (e.shiftKey) {
        const s = Math.max(Math.abs(p[0] - start[0]), Math.abs(p[1] - start[1]));
        q = [start[0] + Math.sign(p[0] - start[0]) * s, start[1] + Math.sign(p[1] - start[1]) * s];
      }
      draft.pts = box(start, q);
    }
  }

  function onpointerup() {
    if (!draft) return;
    const d = draft;
    draft = null;
    start = null;
    // Ignore accidental clicks for drag shapes.
    const [a, b] = [toScreen(d.pts[0]), toScreen(d.pts[d.kind === "rect" || d.kind === "ellipse" || d.kind === "blur" ? 2 : d.pts.length - 1])];
    const moved = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (d.kind !== "pen" && d.kind !== "highlighter" && moved < 4) return;
    change((ed) => ed.markup.push($state.snapshot(d) as Shape));
  }

  function commitText() {
    const t = text.trim();
    const at = typing?.at;
    typing = null;
    if (t && at) change((ed) => ed.markup.push({ ...base("text", [at]), text: t }));
  }

  function onkeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") typing = null;
    else if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      commitText();
    }
  }

  const cursor = $derived(!active ? "default" : editor.tool === "text" ? "text" : editor.tool === "eraser" ? "pointer" : "crosshair");
</script>

<canvas
  bind:this={canvas}
  class="layer"
  class:active
  style:width="{bw}px"
  style:height="{bh}px"
  style:cursor={cursor}
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={onpointerup}
></canvas>

{#if typing}
  <textarea
    bind:this={input}
    bind:value={text}
    class="text-entry"
    rows="1"
    placeholder="Type, then press Enter"
    style:left="{typing.x}px"
    style:top="{typing.y}px"
    style:color={editor.color}
    style:font-size="{sizePx * 5 * scale}px"
    {onkeydown}
    onblur={commitText}
  ></textarea>
{/if}

<style>
  .layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .layer.active {
    pointer-events: auto;
    touch-action: none;
  }
  .text-entry {
    position: absolute;
    min-width: 160px;
    padding: 0;
    border: 1px dashed rgb(255 255 255 / 0.8);
    background: rgb(0 0 0 / 0.15);
    font-family: "Immediate Sans", sans-serif;
    font-weight: 600;
    line-height: 1.2;
    resize: none;
    outline: none;
    field-sizing: content;
  }
</style>
