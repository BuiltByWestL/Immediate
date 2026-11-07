<script lang="ts">
  // Documents: each kind loads its own viewer on first use (they're large), so photos, video
  // and startup never pay for them.
  import { onMount } from "svelte";
  import { leaveGuards } from "../../lib/app.svelte";
  import type { OpenedFile } from "../../lib/types";
  import { closeDoc, confirmLeaveDoc, loadText } from "./doc.svelte";

  let { file }: { file: OpenedFile } = $props();

  const TEXT = new Set(["txt", "log", "json", "xml", "yaml", "yml", "ini", "srt", "vtt"]);
  const view = $derived(
    TEXT.has(file.ext) ? "text" : file.ext === "md" || file.ext === "markdown" ? "markdown" : file.ext === "csv" || file.ext === "tsv" ? "csv" : file.ext === "pdf" ? "pdf" : file.ext === "docx" ? "word" : "sheet",
  );
  const loaders: Record<string, () => Promise<any>> = {
    text: () => import("./TextDoc.svelte"),
    markdown: () => import("./MarkdownDoc.svelte"),
    csv: () => import("./CsvDoc.svelte"),
    pdf: () => import("./PdfDoc.svelte"),
    sheet: () => import("./SheetDoc.svelte"),
    word: () => import("./WordDoc.svelte"),
  };

  $effect(() => {
    if (view === "text" || view === "markdown" || view === "csv") loadText(file.path);
  });

  onMount(() => {
    leaveGuards.add(confirmLeaveDoc);
    return () => {
      leaveGuards.delete(confirmLeaveDoc);
      closeDoc();
    };
  });
</script>

<div class="doc">
  {#await loaders[view]() then m}
    <m.default {file} />
  {:catch e}
    <p class="error">This document couldn’t be opened: {e}</p>
  {/await}
</div>

<style>
  .doc {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
    overflow: hidden;
  }
  .error {
    margin: auto;
    color: var(--danger);
  }
</style>
