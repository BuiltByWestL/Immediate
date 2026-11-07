<script lang="ts">
  // Word documents (docx), view only, converted to a readable page. Layout won't match Word
  // exactly; text, headings, lists, tables and pictures come through.
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import DOMPurify from "dompurify";
  import mammoth from "mammoth";
  import { onMount } from "svelte";
  import type { OpenedFile } from "../../lib/types";

  let { file }: { file: OpenedFile } = $props();
  let html = $state("");
  let error = $state("");

  onMount(async () => {
    try {
      const arrayBuffer = await (await fetch(convertFileSrc(file.path))).arrayBuffer();
      const r = await mammoth.convertToHtml({ arrayBuffer });
      html = DOMPurify.sanitize(r.value);
    } catch (e) {
      error = `This document couldn’t be opened: ${e instanceof Error ? e.message : e}`;
    }
  });

  function onclick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^(https?:|mailto:)/i.test(href)) openUrl(href).catch(() => {});
  }
</script>

<div class="scroll">
  {#if error}
    <p class="error">{error}</p>
  {:else if !html}
    <p class="dim loading">Opening…</p>
  {:else}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <article class="page" {onclick}>{@html html}</article>
  {/if}
</div>

<style>
  .scroll {
    flex: 1;
    overflow: auto;
    padding: var(--s6);
  }
  .page {
    max-width: 816px;
    min-height: 100%;
    margin: 0 auto;
    padding: 72px 80px;
    background: #fff;
    color: #1a1a1a;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.25);
    font-family: Calibri, Carlito, "Segoe UI", var(--font);
    font-size: 15px;
    line-height: 1.5;
    font-variation-settings: normal;
    user-select: text;
    -webkit-user-select: text;
  }
  .page :global(h1),
  .page :global(h2),
  .page :global(h3) {
    margin: 1.2em 0 0.4em;
    line-height: 1.25;
  }
  .page :global(p) {
    margin: 0 0 0.8em;
  }
  .page :global(table) {
    border-collapse: collapse;
    margin: 0 0 1em;
  }
  .page :global(td),
  .page :global(th) {
    padding: 4px 8px;
    border: 1px solid #c8c8c8;
  }
  .page :global(img) {
    max-width: 100%;
  }
  .page :global(a) {
    color: #0b62c4;
  }
  .error {
    color: var(--danger);
  }
  .loading {
    text-align: center;
  }
</style>
