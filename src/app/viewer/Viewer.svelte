<script lang="ts">
  import { ui } from "../../lib/app.svelte";
  import { canPreview, needsDecoding } from "../../lib/formats";
  import type { OpenedFile } from "../../lib/types";
  import ImageView from "./ImageView.svelte";
  import MediaView from "./MediaView.svelte";
  import NoPreview from "./NoPreview.svelte";

  let { file }: { file: OpenedFile } = $props();

  // A failure belongs to the file it happened on, so moving on clears it.
  let failure = $state<{ path: string; reason: string } | null>(null);
  const failed = $derived(failure?.path === file.path ? failure.reason : null);
  const fail = (path: string) => (reason: string) => (failure = { path, reason });

  const NOUN = { image: "pictures", video: "videos", audio: "audio files" };
  const unsupported = $derived(
    canPreview(file) ? null : `Immediate can’t show .${file.ext.toUpperCase()} ${NOUN[file.kind]} yet.`,
  );
</script>

<div class="canvas">
  {#if unsupported || failed}
    <NoPreview {file} reason={(unsupported ?? failed)!} />
  {:else if file.kind === "image"}
    <ImageView path={file.path} rust={needsDecoding(file)} onfail={fail(file.path)} />
  {:else}
    {#key file.path}
      <MediaView {file} onfail={fail(file.path)} />
    {/key}
  {/if}
  {#if ui.loading}
    <div class="busy" aria-hidden="true"></div>
  {/if}
</div>

<style>
  .canvas {
    position: relative;
    flex: 1;
    min-width: 0;
    background: var(--canvas);
    overflow: hidden;
  }
  /* A thin line along the top while a file opens; appears only if opening takes a moment. */
  .busy {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 2px;
    background: var(--accent);
    opacity: 0;
    animation: busy 1s ease-in-out 250ms infinite alternate;
  }
  @keyframes busy {
    from {
      opacity: 0;
    }
    to {
      opacity: 0.8;
    }
  }
</style>
