<script lang="ts">
  // Spreadsheets (xlsx, xls, ods), view only: every sheet, values as the spreadsheet shows them.
  // "Save sheet as CSV" writes the current sheet out for editing elsewhere.
  import { convertFileSrc } from "@tauri-apps/api/core";
  import * as XLSX from "xlsx";
  import { onMount } from "svelte";
  import type { OpenedFile } from "../../lib/types";
  import { saveDoc } from "./doc.svelte";
  import Grid from "./Grid.svelte";

  let { file }: { file: OpenedFile } = $props();
  let wb = $state.raw<XLSX.WorkBook | null>(null);
  let sheet = $state("");
  let error = $state("");

  onMount(async () => {
    try {
      const data = await (await fetch(convertFileSrc(file.path))).arrayBuffer();
      wb = XLSX.read(data, { type: "array", cellDates: true });
      sheet = wb.SheetNames[0] ?? "";
    } catch (e) {
      error = `This spreadsheet couldn’t be opened: ${e instanceof Error ? e.message : e}`;
    }
  });

  const rows = $derived.by(() => {
    if (!wb || !sheet) return [] as string[][];
    const ws = wb.Sheets[sheet];
    return XLSX.utils.sheet_to_json<string[]>(ws, { header: 1, raw: false, defval: "", blankrows: true });
  });

  function saveCsv() {
    if (!wb) return;
    saveDoc("as", XLSX.utils.sheet_to_csv(wb.Sheets[sheet]), "csv");
  }
</script>

<div class="wrap">
  <div class="bar">
    {#if wb && wb.SheetNames.length > 1}
      <div class="tabs" role="tablist">
        {#each wb.SheetNames as name (name)}
          <button type="button" role="tab" aria-selected={sheet === name} onclick={() => (sheet = name)}>{name}</button>
        {/each}
      </div>
    {:else}
      <span class="dim small">{sheet}</span>
    {/if}
    <span class="spacer"></span>
    <span class="dim small num">{Math.max(0, rows.length - 1).toLocaleString()} rows · view only</span>
    <button type="button" class="btn" disabled={!wb} onclick={saveCsv}>Save sheet as CSV…</button>
  </div>
  {#if error}
    <p class="error">{error}</p>
  {:else if wb}
    <Grid {rows} />
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
    min-height: 40px;
    padding: var(--s1) var(--s3);
    background: var(--bg);
    border-bottom: 1px solid var(--line);
  }
  .tabs {
    display: flex;
    gap: 2px;
    overflow-x: auto;
    min-width: 0;
  }
  .tabs button {
    height: 28px;
    padding: 0 var(--s3);
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-dim);
    font-weight: var(--w-medium);
    white-space: nowrap;
    cursor: pointer;
  }
  .tabs button[aria-selected="true"] {
    background: var(--hover);
    color: var(--text);
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--text-sm);
  }
  .error {
    margin: auto;
    color: var(--danger);
  }
</style>
