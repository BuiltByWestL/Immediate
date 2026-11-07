<script lang="ts">
  // Video and audio playback with the webview's own decoder. Formats it can't decode get a clear
  // message here; P3 adds a background proxy through media tools so they play too.
  // The player controls are the webview's own for now (overflow menu hidden). P3 replaces them
  // with Immediate's transport + trim timeline for video, P4 with the waveform editor for audio.
  import { convertFileSrc } from "@tauri-apps/api/core";
  import AudioLines from "@lucide/svelte/icons/audio-lines";
  import { ui, viewer } from "../../lib/app.svelte";
  import type { OpenedFile } from "../../lib/types";

  let { file, onfail }: { file: OpenedFile; onfail: (msg: string) => void } = $props();

  let el = $state<HTMLMediaElement | null>(null);
  const src = $derived(convertFileSrc(file.path));

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
      onfail("This video uses a format the built-in player can’t show yet.");
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
    onfail(
      file.kind === "video"
        ? "This video uses a format the built-in player can’t show yet."
        : "This audio uses a format the built-in player can’t play yet.",
    );
  }
</script>

{#if file.kind === "video"}
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
