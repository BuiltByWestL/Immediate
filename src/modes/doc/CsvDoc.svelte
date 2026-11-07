<script lang="ts">
  import type { OpenedFile } from "../../lib/types";
  import { doc, saveDoc } from "./doc.svelte";
  import { detectDelimiter, parseCsv, serializeCsv, type Table } from "./csv";
  import Grid from "./Grid.svelte";
  import SaveStrip from "./SaveStrip.svelte";

  let { file }: { file: OpenedFile } = $props();

  // Parsed once per load; edits go into the table and back into the text.
  let table = $state<Table | null>(null);
  let parsedFrom = "";
  $effect(() => {
    if (doc.loading || doc.text === parsedFrom) return;
    if (table && doc.text === serializeCsv(table)) return;
    table = parseCsv(doc.text, detectDelimiter(doc.text, file.ext));
    parsedFrom = doc.text;
  });

  function onedit(r: number, c: number, v: string) {
    if (!table || (table.rows[r][c] ?? "") === v) return;
    while (table.rows[r].length <= c) table.rows[r].push("");
    table.rows[r][c] = v;
    doc.text = parsedFrom = serializeCsv(table);
  }

  const NAMES: Record<string, string> = { ",": "Commas", ";": "Semicolons", "\t": "Tabs", "|": "Pipes" };
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
    {#if table}
      <span class="dim small num">{Math.max(0, table.rows.length - 1).toLocaleString()} rows · {NAMES[table.delimiter] ?? table.delimiter} · double-click a cell to edit</span>
    {/if}
  </SaveStrip>
  {#if doc.error}
    <p class="error">{doc.error}</p>
  {:else if table}
    <Grid rows={table.rows} editable {onedit} />
  {/if}
</div>

<style>
  .wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .small {
    font-size: var(--text-sm);
  }
  .error {
    margin: auto;
    color: var(--danger);
  }
</style>
