<script lang="ts">
  // Markdown: a formatted reading view; Edit shows the text beside it, updating live.
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import DOMPurify from "dompurify";
  import { marked } from "marked";
  import type { OpenedFile } from "../../lib/types";
  import { doc, saveDoc } from "./doc.svelte";
  import SaveStrip from "./SaveStrip.svelte";

  let { file }: { file: OpenedFile } = $props();
  let editing = $state(false);

  // Rendered HTML is sanitized: a document can show content, never run code.
  const html = $derived(DOMPurify.sanitize(marked.parse(doc.text, { async: false, gfm: true }) as string));

  // Pictures from disk (relative to the document): Rust checks each and makes it readable.
  let article = $state<HTMLElement | null>(null);
  const resolved = new Map<string, string | null>();
  $effect(() => {
    void html;
    const el = article;
    if (!el) return;
    const imgs = [...el.querySelectorAll("img")].filter((i) => !/^(https?:|data:|blob:)/i.test(i.getAttribute("src") ?? ""));
    const apply = () => {
      for (const img of imgs) {
        const p = resolved.get(img.getAttribute("src") ?? "");
        if (p) img.src = convertFileSrc(p);
      }
    };
    const missing = [...new Set(imgs.map((i) => i.getAttribute("src") ?? ""))].filter((s) => s && !resolved.has(s));
    if (!missing.length) return apply();
    invoke<(string | null)[]>("doc_images", { doc: file.path, srcs: missing })
      .then((paths) => {
        missing.forEach((s, i) => resolved.set(s, paths[i]));
        apply();
      })
      .catch(() => {});
  });

  function onclick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^(https?:|mailto:)/i.test(href)) openUrl(href).catch(() => {});
    else if (href.startsWith("#")) document.getElementById(decodeURIComponent(href.slice(1)))?.scrollIntoView();
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === "s") {
      e.preventDefault();
      e.stopPropagation();
      saveDoc(e.shiftKey ? "as" : "copy");
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="wrap" {onkeydown}>
  <SaveStrip>
    <div class="segmented" role="radiogroup" aria-label="View">
      <button type="button" role="radio" aria-checked={!editing} onclick={() => (editing = false)}>Read</button>
      <button type="button" role="radio" aria-checked={editing} onclick={() => (editing = true)}>Edit</button>
    </div>
  </SaveStrip>
  <div class="panes">
    {#if editing}
      <textarea bind:value={doc.text} spellcheck="true" aria-label="{file.name} text"></textarea>
    {/if}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <article class="prose" bind:this={article} {onclick}>{@html html}</article>
  </div>
</div>

<style>
  .wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .panes {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  textarea {
    flex: 1;
    padding: var(--s5);
    border: 0;
    border-right: 1px solid var(--line);
    outline: none;
    resize: none;
    background: var(--bg);
    color: var(--text);
    font-family: "Cascadia Mono", Consolas, "SF Mono", Menlo, monospace;
    font-size: 13px;
    line-height: 1.6;
    user-select: text;
    -webkit-user-select: text;
  }
  .prose {
    flex: 1;
    overflow: auto;
    padding: var(--s6) max(var(--s6), calc((100% - 72ch) / 2));
    background: var(--bg);
    font-size: 15px;
    line-height: 1.65;
    user-select: text;
    -webkit-user-select: text;
  }
  .prose :global(h1),
  .prose :global(h2),
  .prose :global(h3) {
    margin: 1.4em 0 0.5em;
    font-weight: var(--w-bold);
    line-height: 1.25;
  }
  .prose :global(h1) {
    font-size: 1.8em;
  }
  .prose :global(h2) {
    font-size: 1.4em;
    padding-bottom: 0.25em;
    border-bottom: 1px solid var(--line);
  }
  .prose :global(p),
  .prose :global(ul),
  .prose :global(ol),
  .prose :global(blockquote),
  .prose :global(pre),
  .prose :global(table) {
    margin: 0 0 1em;
  }
  .prose :global(a) {
    color: var(--accent);
  }
  .prose :global(code) {
    padding: 0.1em 0.35em;
    border-radius: 4px;
    background: var(--hover);
    font-family: "Cascadia Mono", Consolas, "SF Mono", Menlo, monospace;
    font-size: 0.88em;
  }
  .prose :global(pre) {
    padding: var(--s3) var(--s4);
    border-radius: var(--radius);
    background: var(--panel);
    overflow: auto;
  }
  .prose :global(pre code) {
    padding: 0;
    background: none;
  }
  .prose :global(blockquote) {
    padding-left: var(--s4);
    border-left: 3px solid var(--line-strong);
    color: var(--text-dim);
  }
  .prose :global(table) {
    border-collapse: collapse;
  }
  .prose :global(th),
  .prose :global(td) {
    padding: var(--s1) var(--s3);
    border: 1px solid var(--line);
  }
  .prose :global(img) {
    max-width: 100%;
  }
  .prose :global(hr) {
    border: 0;
    border-top: 1px solid var(--line);
  }
</style>
