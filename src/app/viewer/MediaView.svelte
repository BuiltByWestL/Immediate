<script lang="ts">
  // Video and audio playback with the webview's own decoder. Formats it can't decode (ProRes,
  // MPEG-2, AVI, AC-3 audio…) play from a stand-in made in the background with media tools.
  // The player controls are the webview's own for now (overflow menu hidden). P3 replaces them
  // with Immediate's transport + trim timeline for video, P4 with the waveform editor for audio.
  import { convertFileSrc } from "@tauri-apps/api/core";
  import AudioLines from "@lucide/svelte/icons/audio-lines";
  import { ui, viewer } from "../../lib/app.svelte";
  import { nativeContainer, needsPreview, preview, previewUrl } from "../../lib/preview.svelte";
  import type { OpenedFile } from "../../lib/types";

  let { file, onfail }: { file: OpenedFile; onfail: (msg: string) => void } = $props();

  let el = $state<HTMLMediaElement | null>(null);
  /** Playing a stand-in (null = the file itself). */
  let proxied = $state<string | null>(null);
  let preparing = $state(false);
  const src = $derived(proxied ?? (nativeContainer(file) ? convertFileSrc(file.path) : null));
  const pct = $derived(Math.round(preview.progress * 100));

  async function usePreview() {
    if (preparing || proxied) return;
    preparing = true;
    try {
      proxied = await previewUrl(file, ui.probe ?? undefined);
    } catch (e) {
      onfail(e instanceof Error ? e.message : String(e));
    } finally {
      preparing = false;
    }
  }

  // Not a container the player opens, or ffprobe found a codec it can't decode: use a stand-in.
  $effect(() => {
    if (!nativeContainer(file)) usePreview();
    else if (ui.probe && needsPreview(file, ui.probe)) usePreview();
  });

  $effect(() => {
    viewer.media = el;
    return () => {
      viewer.media = null;
    };
  });

  function onloadedmetadata() {
    if (!el) return;
    const v = el instanceof HTMLVideoElement ? el : null;
    // A video whose picture can't be decoded still loads, with no size.
    if (file.kind === "video" && v && v.videoWidth === 0) {
      if (!proxied) usePreview();
      else onfail("This video couldn’t be played.");
      return;
    }
    ui.facts = { width: v?.videoWidth, height: v?.videoHeight, duration: el.duration, playable: true };
    // Opening a video or audio file plays it from the start. (Started here rather than with the
    // autoplay attribute, so a video whose picture can't be shown never plays its sound.)
    el.play().catch(() => {
      /* blocked or interrupted: the play button still works */
    });
  }

  function onerror() {
    if (!proxied) {
      usePreview();
      return;
    }
    onfail(
      file.kind === "video"
        ? "This video uses a format the built-in player can’t show yet."
        : "This audio uses a format the built-in player can’t play yet.",
    );
  }
</script>

{#if preparing}
  <div class="preparing" role="status">
    <p>Getting this {file.kind === "video" ? "video" : "audio"} ready to play…{#if pct > 0} <span class="num">{pct}%</span>{/if}</p>
    <div class="bar"><div class="fill" style:width="{pct}%"></div></div>
  </div>
{:else if !src}
  <!-- waiting for the stand-in -->
{:else if file.kind === "video"}
  <!-- svelte-ignore a11y_media_has_caption -->
  <video
    bind:this={el}
    {src}
    controls
    controlslist="nodownload noplaybackrate noremoteplayback"
    disablepictureinpicture
    preload="auto"
    {onloadedmetadata}
    {onerror}
  ></video>
{:else}
  <div class="audio">
    <div class="art" aria-hidden="true"><AudioLines size={40} strokeWidth={1.25} /></div>
    <p class="name">{file.name}</p>
    <audio
      bind:this={el}
      {src}
      controls
      controlslist="nodownload noplaybackrate noremoteplayback"
      preload="auto"
      {onloadedmetadata}
      {onerror}
    ></audio>
  </div>
{/if}

<style>
  .preparing {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: var(--s3);
    color: var(--text-dim);
  }
  .bar {
    width: 220px;
    height: 4px;
    border-radius: 2px;
    background: var(--hover);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 200ms linear;
  }
  video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: transparent;
    outline: none;
  }
  .audio {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: var(--s4);
    padding: var(--s6);
  }
  .art {
    display: grid;
    place-items: center;
    width: 96px;
    height: 96px;
    border-radius: calc(var(--radius) * 2);
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--text-dim);
  }
  .name {
    font-size: var(--text-lg);
    font-weight: var(--w-medium);
    max-width: 60ch;
    text-align: center;
    overflow-wrap: anywhere;
  }
  audio {
    width: min(480px, 80vw);
  }
</style>
