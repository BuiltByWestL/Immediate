<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Copy from "@lucide/svelte/icons/copy";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import { copyPath, revealCurrent, ui } from "../lib/app.svelte";
  import { formatDate, formatDuration, formatSize, kindName } from "../lib/formats";
  import type { OpenedFile } from "../lib/types";

  let { file }: { file: OpenedFile } = $props();
  let down: { x: number; y: number } | null = null;

  const rows = $derived.by(() => {
    const f = ui.facts;
    const r: [string, string][] = [["Kind", `${kindName(file)} · .${file.ext.toUpperCase()}`]];
    if (f.width && f.height) {
      const mp = (f.width * f.height) / 1e6;
      r.push(["Dimensions", `${f.width} × ${f.height}` + (file.kind === "image" && mp >= 0.1 ? ` · ${mp.toFixed(1)} MP` : "")]);
    }
    if (f.duration) r.push(["Duration", formatDuration(f.duration)]);
    r.push(["Size", formatSize(file.size)]);
    if (file.modified) r.push(["Modified", formatDate(file.modified)]);
    if (ui.details?.created) r.push(["Created", formatDate(ui.details.created)]);
    return r;
  });

  const location = $derived.by(() => {
    const l = ui.details?.location;
    if (!l) return null;
    const [lat, lon] = l;
    return `${Math.abs(lat).toFixed(4)}° ${lat < 0 ? "S" : "N"}, ${Math.abs(lon).toFixed(4)}° ${lon < 0 ? "W" : "E"}`;
  });
</script>

<aside class="panel" aria-label="File info">
  <section>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <h2
      class="title"
      title="Drag into another app to share this file"
      onpointerdown={(e) => (down = e.button === 0 ? { x: e.clientX, y: e.clientY } : null)}
      onpointermove={(e) => {
        if (down && Math.hypot(e.clientX - down.x, e.clientY - down.y) > 5) {
          down = null;
          invoke("start_drag", { path: file.path }).catch(() => {});
        }
      }}
      onpointerup={() => (down = null)}
    >
      {file.name}
    </h2>
    <p class="folder dim" title={file.dir}>{file.dir}</p>
  </section>

  <section>
    <dl>
      {#each rows as [k, v] (k)}
        <dt>{k}</dt>
        <dd class="num">{v}</dd>
      {/each}
    </dl>
  </section>

  {#if ui.details?.exif.length || location}
    <section>
      <h3>Camera</h3>
      <dl>
        {#each ui.details?.exif ?? [] as [k, v] (k)}
          <dt>{k}</dt>
          <dd class="num">{v}</dd>
        {/each}
        {#if location}
          <dt>Location</dt>
          <dd class="num">{location}</dd>
        {/if}
      </dl>
    </section>
  {/if}

  <section class="actions">
    <button type="button" class="btn" onclick={revealCurrent}><FolderOpen size={16} strokeWidth={1.5} />Show in folder</button>
    <button type="button" class="btn" onclick={copyPath}><Copy size={16} strokeWidth={1.5} />Copy location</button>
  </section>
</aside>

<style>
  .panel {
    flex: none;
    width: 288px;
    overflow-y: auto;
    background: var(--panel);
    border-left: 1px solid var(--line);
  }
  section {
    padding: var(--s4);
    border-bottom: 1px solid var(--line);
  }
  section:last-child {
    border-bottom: 0;
  }
  .title {
    cursor: grab;
    font-size: var(--text-md);
    font-weight: var(--w-medium);
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }
  .folder {
    margin-top: var(--s1);
    font-size: var(--text-sm);
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }
  h3 {
    margin-bottom: var(--s2);
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
    color: var(--text-dim);
  }
  dl {
    display: grid;
    grid-template-columns: 88px 1fr;
    gap: var(--s1) var(--s3);
    margin: 0;
  }
  dt {
    color: var(--text-dim);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }
  .actions {
    display: grid;
    gap: var(--s2);
  }
  .actions .btn {
    justify-content: flex-start;
  }
</style>
