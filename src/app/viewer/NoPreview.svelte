<script lang="ts">
  import ImageOff from "@lucide/svelte/icons/image-off";
  import Film from "@lucide/svelte/icons/film";
  import AudioLines from "@lucide/svelte/icons/audio-lines";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import { revealCurrent } from "../../lib/app.svelte";
  import { formatSize } from "../../lib/formats";
  import type { OpenedFile } from "../../lib/types";

  let { file, reason }: { file: OpenedFile; reason: string } = $props();
  const Icon = $derived(file.kind === "image" ? ImageOff : file.kind === "video" ? Film : AudioLines);
</script>

<div class="wrap">
  <div class="card">
    <div class="icon" aria-hidden="true"><Icon size={28} strokeWidth={1.25} /></div>
    <p class="name">{file.name}</p>
    <p class="meta dim"><span class="num">{formatSize(file.size)}</span> · .{file.ext}</p>
    <p class="reason">{reason}</p>
    <button type="button" class="btn" onclick={revealCurrent}>
      <FolderOpen size={16} strokeWidth={1.5} />Show in folder
    </button>
  </div>
</div>

<style>
  .wrap {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: var(--s6);
  }
  .card {
    display: grid;
    justify-items: center;
    gap: var(--s2);
    max-width: 380px;
    text-align: center;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    margin-bottom: var(--s2);
    border-radius: calc(var(--radius) * 2);
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--text-dim);
  }
  .name {
    font-size: var(--text-lg);
    font-weight: var(--w-medium);
    overflow-wrap: anywhere;
  }
  .reason {
    margin: var(--s2) 0 var(--s3);
    color: var(--text-dim);
  }
</style>
