<script lang="ts">
  // PDF with Mozilla's PDF.js: pages render as they scroll into view, text can be selected and
  // copied, Ctrl+F searches. Fits the window width; Ctrl +/- zooms.
  import { convertFileSrc } from "@tauri-apps/api/core";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import Search from "@lucide/svelte/icons/search";
  import * as pdfjs from "pdfjs-dist";
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import "pdfjs-dist/web/pdf_viewer.css";
  import { onMount } from "svelte";
  import type { OpenedFile } from "../../lib/types";

  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

  let { file }: { file: OpenedFile } = $props();

  let scroller = $state<HTMLDivElement>();
  let task: pdfjs.PDFDocumentLoadingTask | null = null;
  let pdf = $state.raw<pdfjs.PDFDocumentProxy | null>(null);
  let sizes = $state<{ w: number; h: number }[]>([]);
  let zoom = $state(1);
  let fitWidth = $state(true);
  let width = $state(0);
  let error = $state("");
  let current = $state(1);
  let query = $state("");
  let finding = $state(false);
  let found = $state("");
  const texts = new Map<number, string>();
  const rendered = new Map<number, { scale: number; task?: { cancel(): void } }>();
  const pageEls: HTMLDivElement[] = $state([]);

  const scale = $derived(fitWidth && sizes[0] ? Math.max(0.2, (width - 48) / sizes[0].w) : zoom);

  onMount(() => {
    let destroyed = false;
    (async () => {
      try {
        const data = await (await fetch(convertFileSrc(file.path))).arrayBuffer();
        task = pdfjs.getDocument({ data, useSystemFonts: true });
        const doc = await task.promise;
        if (destroyed) return void task.destroy();
        const s: { w: number; h: number }[] = [];
        for (let i = 1; i <= doc.numPages; i++) {
          const v = (await doc.getPage(i)).getViewport({ scale: 1 });
          s.push({ w: v.width, h: v.height });
        }
        sizes = s;
        pdf = doc;
      } catch (e) {
        error = `This PDF couldn’t be opened: ${e instanceof Error ? e.message : e}`;
      }
    })();
    return () => {
      destroyed = true;
      task?.destroy();
    };
  });

  // Render pages near the viewport (and re-render after zooming).
  let raf = 0;
  function update() {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => {
      if (!pdf || !scroller) return;
      const top = scroller.scrollTop;
      const bottom = top + scroller.clientHeight;
      pageEls.forEach((el, i) => {
        if (!el) return;
        const a = el.offsetTop;
        const b = a + el.offsetHeight;
        const mid = top + (scroller?.clientHeight ?? 0) / 2;
        if (a <= mid && b > mid) current = i + 1;
        if (b > top - 800 && a < bottom + 800) renderPage(i + 1, el);
      });
    });
  }
  $effect(() => {
    void [scale, pdf, sizes.length];
    update();
  });

  async function renderPage(n: number, el: HTMLDivElement) {
    if (!pdf) return;
    const r = rendered.get(n);
    if (r && r.scale === scale) return;
    r?.task?.cancel();
    const entry: { scale: number; task?: { cancel(): void } } = { scale };
    rendered.set(n, entry);
    const page = await pdf.getPage(n);
    const viewport = page.getViewport({ scale });
    const dpr = window.devicePixelRatio || 1;
    const canvas = document.createElement("canvas");
    canvas.width = Math.floor(viewport.width * dpr);
    canvas.height = Math.floor(viewport.height * dpr);
    canvas.style.width = `${viewport.width}px`;
    canvas.style.height = `${viewport.height}px`;
    const task = page.render({ canvas, canvasContext: canvas.getContext("2d")!, viewport, transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined });
    entry.task = task;
    try {
      await task.promise;
    } catch {
      return; // cancelled by a newer zoom
    }
    if (rendered.get(n) !== entry) return;
    const text = document.createElement("div");
    text.className = "textLayer";
    el.replaceChildren(canvas, text);
    el.style.setProperty("--scale-factor", String(scale));
    new pdfjs.TextLayer({ textContentSource: page.streamTextContent(), container: text, viewport }).render().catch(() => {});
  }

  async function textOf(n: number) {
    if (!texts.has(n) && pdf) {
      const c = await (await pdf.getPage(n)).getTextContent();
      texts.set(n, c.items.map((i) => ("str" in i ? i.str : "")).join(" ").toLowerCase());
    }
    return texts.get(n) ?? "";
  }

  async function findNext() {
    if (!pdf || !query) return;
    const q = query.toLowerCase();
    for (let k = 0; k < pdf.numPages; k++) {
      const n = ((current - 1 + k + (found ? 1 : 0)) % pdf.numPages) + 1;
      if ((await textOf(n)).includes(q)) {
        found = `Found on page ${n}`;
        pageEls[n - 1]?.scrollIntoView({ block: "start" });
        current = n;
        return;
      }
    }
    found = "Not found";
  }

  function setZoom(z: number) {
    fitWidth = false;
    zoom = Math.min(5, Math.max(0.25, z));
  }

  function onkeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key === "f") {
      e.preventDefault();
      finding = true;
    } else if (mod && (e.key === "=" || e.key === "+")) {
      e.preventDefault();
      setZoom(scale * 1.2);
    } else if (mod && e.key === "-") {
      e.preventDefault();
      setZoom(scale / 1.2);
    } else if (mod && e.key === "0") {
      e.preventDefault();
      fitWidth = true;
    } else return;
    e.stopPropagation();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="wrap" {onkeydown}>
  <div class="bar">
    <span class="dim small num">{pdf ? `Page ${current} of ${pdf.numPages}` : "Opening…"}</span>
    <span class="spacer"></span>
    {#if finding}
      <!-- svelte-ignore a11y_autofocus -->
      <input class="field" placeholder="Find in document" bind:value={query} autofocus onkeydown={(e) => e.key === "Enter" && (e.preventDefault(), findNext())} oninput={() => (found = "")} />
      <span class="dim small">{found}</span>
    {:else}
      <button type="button" class="icon-btn" aria-label="Find" title="Find (Ctrl+F)" onclick={() => (finding = true)}><Search size={16} strokeWidth={1.5} /></button>
    {/if}
    <button type="button" class="icon-btn" aria-label="Zoom out" title="Zoom out (Ctrl −)" onclick={() => setZoom(scale / 1.2)}><Minus size={16} strokeWidth={1.5} /></button>
    <button type="button" class="zoom num" title="Fit width (Ctrl+0)" onclick={() => (fitWidth = true)}>{Math.round(scale * 100)}%</button>
    <button type="button" class="icon-btn" aria-label="Zoom in" title="Zoom in (Ctrl +)" onclick={() => setZoom(scale * 1.2)}><Plus size={16} strokeWidth={1.5} /></button>
  </div>
  {#if error}
    <p class="error">{error}</p>
  {:else}
    <div class="pages" bind:this={scroller} bind:clientWidth={width} onscroll={update}>
      {#each sizes as s, i (i)}
        <div class="page" bind:this={pageEls[i]} style:width="{s.w * scale}px" style:height="{s.h * scale}px"></div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .bar {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--s2);
    height: 40px;
    padding: 0 var(--s3);
    background: var(--bg);
    border-bottom: 1px solid var(--line);
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--text-sm);
  }
  .zoom {
    min-width: 52px;
    height: 24px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
  }
  .pages {
    flex: 1;
    overflow: auto;
    padding: var(--s6) var(--s6) var(--s8);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s4);
  }
  .page {
    position: relative;
    flex: none;
    background: #fff;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.25);
  }
  .page :global(canvas) {
    display: block;
  }
  .error {
    margin: auto;
    color: var(--danger);
  }
</style>
