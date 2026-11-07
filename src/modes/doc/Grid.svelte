<script lang="ts">
  // A table for CSV and spreadsheets. Only the rows on screen exist, so huge files stay smooth.
  // The first row is the header (sticky; click to sort the view). Double-click a cell to edit.
  let {
    rows,
    editable = false,
    onedit,
  }: { rows: string[][]; editable?: boolean; onedit?: (r: number, c: number, v: string) => void } = $props();

  const ROW = 28;
  let box: HTMLDivElement;
  let top = $state(0);
  let height = $state(0);
  let sort = $state<{ col: number; dir: 1 | -1 } | null>(null);
  let editing = $state<{ r: number; c: number; value: string } | null>(null);

  const cols = $derived(rows.reduce((n, r) => Math.max(n, r.length), 0));
  const widths = $derived.by(() => {
    const w: number[] = [];
    const sample = rows.slice(0, 300);
    for (let c = 0; c < cols; c++) {
      const len = sample.reduce((m, r) => Math.max(m, (r[c] ?? "").length), 0);
      w.push(Math.min(360, Math.max(64, len * 7.4 + 20)));
    }
    return w;
  });
  const total = $derived(widths.reduce((a, b) => a + b, 0) + 56);

  /** Row order on screen (index into rows; header stays first). */
  const order = $derived.by(() => {
    const idx = rows.slice(1).map((_, i) => i + 1);
    if (!sort) return idx;
    const { col, dir } = sort;
    const num = (v: string) => (v.trim() !== "" && !isNaN(Number(v)) ? Number(v) : null);
    return idx.sort((a, b) => {
      const x = rows[a][col] ?? "";
      const y = rows[b][col] ?? "";
      const nx = num(x);
      const ny = num(y);
      return dir * (nx !== null && ny !== null ? nx - ny : x.localeCompare(y, undefined, { numeric: true, sensitivity: "base" }));
    });
  });

  const first = $derived(Math.max(0, Math.floor(top / ROW) - 8));
  const last = $derived(Math.min(order.length, Math.ceil((top + height) / ROW) + 8));

  function sortBy(c: number) {
    sort = sort?.col === c ? (sort.dir === 1 ? { col: c, dir: -1 } : null) : { col: c, dir: 1 };
  }

  function startEdit(r: number, c: number) {
    if (!editable) return;
    editing = { r, c, value: rows[r][c] ?? "" };
  }
  function commitEdit() {
    if (!editing) return;
    onedit?.(editing.r, editing.c, editing.value);
    editing = null;
  }
  const letter = (c: number) => {
    let s = "";
    for (let n = c + 1; n > 0; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + ((n - 1) % 26)) + s;
    return s;
  };
</script>

<div class="grid" bind:this={box} bind:clientHeight={height} onscroll={() => (top = box.scrollTop)}>
  <div class="inner" style:width="{total}px" style:height="{(order.length + 1) * ROW}px">
    <div class="row head" style:top="{top}px">
      <span class="num gutter"></span>
      {#each Array(cols) as _, c (c)}
        <button type="button" class="cell" style:width="{widths[c]}px" onclick={() => sortBy(c)} title="Sort by this column">
          <span class="text">{rows[0]?.[c] || letter(c)}</span>
          {#if sort?.col === c}<span class="dim">{sort.dir === 1 ? "▲" : "▼"}</span>{/if}
        </button>
      {/each}
    </div>
    {#each order.slice(first, last) as r, k (r)}
      <div class="row" style:top="{(first + k + 1) * ROW}px">
        <span class="num gutter dim">{r}</span>
        {#each Array(cols) as _, c (c)}
          {#if editing && editing.r === r && editing.c === c}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="cell edit"
              style:width="{widths[c]}px"
              bind:value={editing.value}
              autofocus
              onblur={commitEdit}
              onkeydown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") commitEdit();
                else if (e.key === "Escape") editing = null;
              }}
            />
          {:else}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <span class="cell" class:editable style:width="{widths[c]}px" ondblclick={() => startEdit(r, c)} title={rows[r][c]}>{rows[r][c] ?? ""}</span>
          {/if}
        {/each}
      </div>
    {/each}
  </div>
</div>

<style>
  .grid {
    flex: 1;
    overflow: auto;
    background: var(--bg);
    font-size: var(--text-sm);
  }
  .inner {
    position: relative;
  }
  .row {
    position: absolute;
    left: 0;
    display: flex;
    height: 28px;
    border-bottom: 1px solid var(--line);
  }
  .head {
    z-index: 2;
    background: var(--panel);
    font-weight: var(--w-medium);
  }
  .gutter {
    position: sticky;
    left: 0;
    flex: none;
    width: 56px;
    padding: 0 var(--s2);
    line-height: 28px;
    text-align: right;
    background: var(--panel);
    border-right: 1px solid var(--line);
  }
  .cell {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--s1);
    padding: 0 var(--s2);
    border: 0;
    border-right: 1px solid var(--line);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    user-select: text;
    -webkit-user-select: text;
  }
  .head .cell {
    cursor: pointer;
    user-select: none;
  }
  .text {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .editable {
    cursor: cell;
  }
  .edit {
    height: 27px;
    outline: 2px solid var(--focus);
    outline-offset: -2px;
    background: var(--raised);
  }
</style>
