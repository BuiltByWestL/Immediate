<script lang="ts">
  // Plain text and code: edit in place, find & replace (Ctrl+F / Ctrl+H), Save keeps the file's
  // encoding and line endings.
  import Search from "@lucide/svelte/icons/search";
  import { onMount } from "svelte";
  import type { OpenedFile } from "../../lib/types";
  import { doc, saveDoc } from "./doc.svelte";
  import SaveStrip from "./SaveStrip.svelte";

  let { file, area = $bindable() }: { file: OpenedFile; area?: HTMLTextAreaElement } = $props();

  let finding = $state(false);
  let replacing = $state(false);
  let query = $state("");
  let replacement = $state("");
  let input = $state<HTMLInputElement | null>(null);
  let hits = $derived(query ? countHits(doc.text, query) : 0);

  function countHits(t: string, q: string) {
    let n = 0;
    const lt = t.toLowerCase();
    const lq = q.toLowerCase();
    for (let i = lt.indexOf(lq); i >= 0; i = lt.indexOf(lq, i + Math.max(1, lq.length))) n++;
    return n;
  }

  function findNext(back = false) {
    if (!area || !query) return;
    const t = doc.text.toLowerCase();
    const q = query.toLowerCase();
    let i = back ? t.lastIndexOf(q, area.selectionStart - 1) : t.indexOf(q, area.selectionEnd);
    if (i < 0) i = back ? t.lastIndexOf(q) : t.indexOf(q); // wrap around
    if (i < 0) return;
    area.focus();
    area.setSelectionRange(i, i + query.length);
    // Scroll the match into view.
    const before = doc.text.slice(0, i).split("\n").length;
    const lh = parseFloat(getComputedStyle(area).lineHeight) || 18;
    area.scrollTop = Math.max(0, (before - 5) * lh);
    input?.focus();
  }

  function replaceOne() {
    if (!area || !query) return;
    const sel = doc.text.slice(area.selectionStart, area.selectionEnd);
    if (sel.toLowerCase() === query.toLowerCase()) {
      const at = area.selectionStart;
      doc.text = doc.text.slice(0, at) + replacement + doc.text.slice(area.selectionEnd);
      queueMicrotask(() => area!.setSelectionRange(at + replacement.length, at + replacement.length));
    }
    queueMicrotask(() => findNext());
  }

  function replaceAll() {
    if (!query) return;
    const re = new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
    doc.text = doc.text.replace(re, () => replacement);
  }

  function onkeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && (e.key === "f" || e.key === "h")) {
      e.preventDefault();
      finding = true;
      replacing = e.key === "h";
      const sel = area ? doc.text.slice(area.selectionStart, area.selectionEnd) : "";
      if (sel && !sel.includes("\n")) query = sel;
      queueMicrotask(() => input?.select());
    } else if (mod && e.key === "s") {
      e.preventDefault();
      saveDoc(e.shiftKey ? "as" : "copy");
    } else if (e.key === "Escape" && finding) {
      finding = false;
      area?.focus();
    } else return;
    e.stopPropagation();
  }

  onMount(() => area?.focus());

  const ENC: Record<string, string> = { "utf-8": "UTF-8", "utf-8-bom": "UTF-8 with BOM", "utf-16le": "UTF-16", "utf-16be": "UTF-16 BE", "windows-1252": "Windows-1252" };
  const lines = $derived(doc.text ? doc.text.split("\n").length : 0);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="wrap" {onkeydown}>
  <SaveStrip>
    <button type="button" class="icon-btn" title="Find (Ctrl+F) · Replace (Ctrl+H)" aria-label="Find" aria-pressed={finding} onclick={() => ((finding = !finding), queueMicrotask(() => input?.focus()))}>
      <Search size={16} strokeWidth={1.5} />
    </button>
    {#if finding}
      <input bind:this={input} class="field find" placeholder="Find" bind:value={query} onkeydown={(e) => e.key === "Enter" && (e.preventDefault(), findNext(e.shiftKey))} />
      <span class="dim small num">{query ? `${hits} found` : ""}</span>
      {#if replacing}
        <input class="field find" placeholder="Replace with" bind:value={replacement} onkeydown={(e) => e.key === "Enter" && (e.preventDefault(), replaceOne())} />
        <button type="button" class="btn" disabled={!hits} onclick={replaceOne}>Replace</button>
        <button type="button" class="btn" disabled={!hits} onclick={replaceAll}>All</button>
      {:else}
        <button type="button" class="btn" onclick={() => (replacing = true)}>Replace…</button>
      {/if}
    {:else}
      <span class="dim small num">{lines} lines · {ENC[doc.encoding] ?? doc.encoding} · {doc.eol === "\r\n" ? "Windows line endings" : "Unix line endings"}</span>
    {/if}
  </SaveStrip>
  {#if doc.error}
    <p class="error">{doc.error}</p>
  {:else}
    <textarea bind:this={area} bind:value={doc.text} class:wrap-lines={file.ext === "txt"} spellcheck={file.ext === "txt" || file.ext === "md"} aria-label={file.name} disabled={doc.loading}></textarea>
  {/if}
</div>

<style>
  .wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  textarea {
    flex: 1;
    width: 100%;
    padding: var(--s4) var(--s5);
    border: 0;
    outline: none;
    resize: none;
    background: var(--bg);
    color: var(--text);
    font-family: "Cascadia Mono", Consolas, "SF Mono", Menlo, "DejaVu Sans Mono", monospace;
    font-size: 13px;
    line-height: 1.55;
    tab-size: 4;
    white-space: pre;
    overflow: auto;
    user-select: text;
    -webkit-user-select: text;
  }
  textarea.wrap-lines {
    white-space: pre-wrap;
  }
  .find {
    width: 180px;
  }
  .small {
    font-size: var(--text-sm);
    white-space: nowrap;
  }
  .error {
    margin: auto;
    color: var(--danger);
  }
</style>
