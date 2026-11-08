<script lang="ts">
  // Video and audio editor: player, transport, timeline, and the panel of edits.
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import AudioLines from "@lucide/svelte/icons/audio-lines";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import { onMount } from "svelte";
  import { toast, viewer } from "../../lib/app.svelte";
  import { needsPreview, preview, previewUrl } from "../../lib/preview.svelte";
  import type { OpenedFile } from "../../lib/types";
  import MediaPanel from "./MediaPanel.svelte";
  import { mediaKeys } from "./keys";
  import { changeMedia, commitMedia as commit, fmtTime, keptSegments, media, nextKept, totalLength } from "./session.svelte";
  import Timeline from "./Timeline.svelte";

  let { file }: { file: OpenedFile } = $props();

  const isVideo = $derived((media.probe?.streams ?? []).some((s: any) => s.codec_type === "video" && s.disposition?.attached_pic !== 1));
  const fps = $derived.by(() => {
    const v = (media.probe?.streams ?? []).find((s: any) => s.codec_type === "video");
    const [n, d] = String(v?.avg_frame_rate ?? "30/1").split("/").map(Number);
    return d ? n / d : 30;
  });

  let el = $state<HTMLVideoElement | HTMLAudioElement | null>(null);
  /** The built-in player can't decode this: show frames rendered by ffmpeg instead. */
  let stills = $state(false);
  let still = $state<string | null>(null);

  /** A playable stand-in for formats the built-in player can't decode. */
  let proxied = $state<string | null>(null);
  let preparing = $state(false);
  const src = $derived(proxied ?? convertFileSrc(file.path));

  async function usePreview() {
    if (preparing || proxied) return;
    preparing = true;
    stills = true;
    showStill(media.time);
    try {
      proxied = await previewUrl(file, media.probe);
      stills = false;
    } catch {
      /* stay on still frames: editing still works */
    } finally {
      preparing = false;
    }
  }
  const kept = $derived(keptSegments());

  // Follow playback, skipping the parts that are cut out.
  let raf = 0;
  function tick() {
    if (el && !el.paused) {
      const t = el.currentTime;
      const next = nextKept(t, kept);
      if (next === null) {
        el.pause();
        el.currentTime = media.edit.trim.end;
      } else if (next > t + 0.01) {
        el.currentTime = next;
      }
      media.time = el.currentTime;
      if (audio) audio.fade.gain.setTargetAtTime(fadeAt(el.currentTime), audio.ctx.currentTime, 0.015);
    }
    raf = requestAnimationFrame(tick);
  }

  function seek(t: number) {
    media.time = Math.max(0, Math.min(media.duration, t));
    if (el && !stills) el.currentTime = media.time;
    if (stills) showStill(media.time);
  }

  let stillReq = 0;
  async function showStill(t: number) {
    const req = ++stillReq;
    try {
      const buf = await invoke<ArrayBuffer>("media_frame", { path: file.path, time: t, height: 720 });
      if (req !== stillReq) return;
      if (still) URL.revokeObjectURL(still);
      still = URL.createObjectURL(new Blob([buf], { type: "image/jpeg" }));
    } catch {
      /* leave the last frame */
    }
  }

  function togglePlay() {
    if (!el || stills) return;
    if (el.paused) {
      // Play from the start of the kept part if we're at (or past) the end.
      const next = nextKept(el.currentTime, kept);
      if (next === null || el.currentTime >= media.edit.trim.end - 0.05) el.currentTime = kept[0]?.start ?? 0;
      else el.currentTime = next;
      el.play().catch(() => {});
    } else el.pause();
  }

  onMount(() => {
    raf = requestAnimationFrame(tick);
    viewer.media = null;
    if (needsPreview(file, media.probe)) usePreview();
    // Keyboard: J/K/L, frame steps, Q (or I) / E (or O) to trim to the playhead, W (or Delete,
    // handled in App) to cut out the selection.
    mediaKeys.handler = (e: KeyboardEvent) => {
      const k = e.key.toLowerCase();
      const step = e.shiftKey ? 1 : 1 / fps;
      if (k === " " || k === "k") togglePlay();
      else if (k === "j") seek(media.time - 5);
      else if (k === "l") seek(media.time + 5);
      else if (k === "arrowleft") seek(media.time - step);
      else if (k === "arrowright") seek(media.time + step);
      else if (k === "q" || k === "i") {
        media.edit.trim.start = Math.min(media.time, media.edit.trim.end - 0.1);
        commit();
      } else if (k === "e" || k === "o") {
        media.edit.trim.end = Math.max(media.time, media.edit.trim.start + 0.1);
        commit();
      } else if (k === "w") {
        const s = media.selection;
        if (!s) toast("Drag across a part of the timeline first, then press W to cut it out.");
        else {
          changeMedia((ed) => ed.cuts.push({ ...s }));
          media.selection = null;
          seek(s.end);
        }
      } else return false;
      return true;
    };
    return () => {
      cancelAnimationFrame(raf);
      mediaKeys.handler = null;
      if (still) URL.revokeObjectURL(still);
    };
  });

  // Live sound: volume, fades, loudness, tone and leveling are heard while playing, through Web
  // Audio nodes that follow the filters used when saving. Only noise reduction is save-only.
  let audio: { ctx: AudioContext; gain: GainNode; fade: GainNode; eq: BiquadFilterNode[]; comp: DynamicsCompressorNode; out: AudioNode } | null = null;

  /** Fade level at source time t, measured along the kept parts like the saved file. */
  function fadeAt(t: number) {
    const { fadeIn, fadeOut } = media.edit;
    if (!fadeIn && !fadeOut) return 1;
    let pos = 0;
    for (const s of kept) {
      if (t >= s.end) pos += s.end - s.start;
      else {
        pos += Math.max(0, t - s.start);
        break;
      }
    }
    const total = totalLength(kept);
    let g = 1;
    if (fadeIn) g *= Math.min(1, pos / Math.min(fadeIn, total));
    if (fadeOut) g *= Math.min(1, (total - pos) / Math.min(fadeOut, total));
    return Math.max(0, g);
  }

  // "Even out loudness": measured once (the same loudnorm measurement the save uses), heard as
  // the gain that brings the track to -16 LUFS without passing -1.5 dB true peak.
  let loudness = $state<{ i: number; tp: number } | null>(null);
  let measuring = false;
  $effect(() => {
    if (!media.edit.normalize || loudness || measuring) return;
    measuring = true;
    invoke<[number, number]>("media_loudness", { path: file.path })
      .then(([i, tp]) => (loudness = { i, tp }))
      .catch(() => {})
      .finally(() => (measuring = false));
  });
  // Saving levels loudness after the volume change, so with it on the volume slider doesn't matter.
  const liveGainDb = $derived(media.edit.normalize && loudness ? Math.max(-30, Math.min(30, -16 - loudness.i, -1.5 - loudness.tp)) : media.edit.gain);
  function connectAudio() {
    if (!el || audio) return;
    try {
      const ctx = new AudioContext();
      const src = ctx.createMediaElementSource(el);
      const gain = ctx.createGain();
      const fade = ctx.createGain();
      const mk = (type: BiquadFilterType, f: number) => {
        const b = ctx.createBiquadFilter();
        b.type = type;
        b.frequency.value = f;
        b.Q.value = 1;
        return b;
      };
      const eq = [mk("lowshelf", 100), mk("peaking", 1000), mk("highshelf", 8000)];
      const comp = ctx.createDynamicsCompressor();
      comp.threshold.value = -20;
      comp.ratio.value = 3;
      comp.attack.value = 0.015;
      comp.release.value = 0.25;
      src.connect(gain).connect(fade).connect(eq[0]).connect(eq[1]).connect(eq[2]);
      gain.gain.value = Math.pow(10, liveGainDb / 20);
      fade.gain.value = fadeAt(el.currentTime);
      audio = { ctx, gain, fade, eq, comp, out: eq[2] };
      routeCompressor();
    } catch {
      /* plays without live effects */
    }
  }
  function routeCompressor() {
    if (!audio) return;
    audio.out.disconnect();
    audio.comp.disconnect();
    if (media.edit.compress) audio.out.connect(audio.comp).connect(audio.ctx.destination);
    else audio.out.connect(audio.ctx.destination);
  }
  $effect(() => {
    const e = media.edit;
    if (!audio) return;
    audio.gain.gain.setTargetAtTime(Math.pow(10, liveGainDb / 20), audio.ctx.currentTime, 0.02);
    e.eq.forEach((g, i) => (audio!.eq[i].gain.value = g));
    void e.compress;
    routeCompressor();
  });
  $effect(() => () => audio?.ctx.close());

  function onerror() {
    if (!proxied) usePreview();
    else {
      stills = true;
      showStill(media.time);
    }
  }
  function onloadedmetadata() {
    if (el instanceof HTMLVideoElement && el.videoWidth === 0 && isVideo) onerror();
    else if (el) {
      el.currentTime = media.edit.trim.start;
      // Opening for editing plays from the start, like viewing does.
      el.play().catch(() => {});
    }
  }
</script>

<div class="editor">
  <div class="main">
    <div class="stage">
      {#if isVideo && !stills}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          bind:this={el}
          {src}
          preload="auto"
          muted={media.edit.mute}
          crossorigin="anonymous"
          onplay={() => {
            media.playing = true;
            connectAudio();
            audio?.ctx.resume();
          }}
          onpause={() => (media.playing = false)}
          {onloadedmetadata}
          {onerror}
          onclick={togglePlay}
        ></video>
      {:else if isVideo && stills}
        {#if still}<img src={still} alt="" />{/if}
        <p class="note">
          {#if preparing}Getting this video ready to play… <span class="num">{Math.round(preview.progress * 100)}%</span> You can trim and cut already.
          {:else}This video can’t play inside Immediate. You can still trim and cut it: the picture follows where you click.{/if}
        </p>
      {:else}
        <div class="audio-art" aria-hidden="true"><AudioLines size={40} strokeWidth={1.25} /></div>
        <audio
          bind:this={el}
          {src}
          preload="auto"
          muted={media.edit.mute}
          crossorigin="anonymous"
          onplay={() => {
            media.playing = true;
            connectAudio();
            audio?.ctx.resume();
          }}
          onpause={() => (media.playing = false)}
          {onloadedmetadata}
          onerror={() => (proxied ? (stills = true) : usePreview())}
        ></audio>
      {/if}
    </div>

    <div class="transport">
      <button type="button" class="icon-btn play" aria-label={media.playing ? "Pause" : "Play"} title="Play / pause (Space)" disabled={stills} onclick={togglePlay}>
        {#if media.playing}<Pause size={18} strokeWidth={1.75} />{:else}<Play size={18} strokeWidth={1.75} />{/if}
      </button>
      <span class="num time">{fmtTime(media.time, true)} <span class="dim">/ {fmtTime(media.duration, true)}</span></span>
      <span class="spacer"></span>
      <span class="num dim">Keeping <span class="kept">{fmtTime(totalLength(kept), true)}</span></span>
    </div>

    <div class="timeline-wrap">
      <Timeline path={file.path} video={isVideo} {seek} />
    </div>
  </div>
  <MediaPanel {isVideo} {seek} />
</div>

<style>
  .editor {
    flex: 1;
    display: flex;
    min-width: 0;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
  }
  video,
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .note {
    position: absolute;
    bottom: var(--s3);
    left: 50%;
    transform: translateX(-50%);
    padding: var(--s1) var(--s3);
    border-radius: var(--radius);
    background: var(--raised);
    border: 1px solid var(--line);
    font-size: var(--text-sm);
    max-width: 90%;
    text-align: center;
  }
  .audio-art {
    display: grid;
    place-items: center;
    width: 96px;
    height: 96px;
    border-radius: calc(var(--radius) * 2);
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--text-dim);
  }
  .transport {
    display: flex;
    align-items: center;
    gap: var(--s3);
    height: 40px;
    padding: 0 var(--s3);
    background: var(--bg);
    border-top: 1px solid var(--line);
  }
  .time {
    font-weight: var(--w-medium);
  }
  .kept {
    color: var(--text);
    font-weight: var(--w-medium);
  }
  .spacer {
    flex: 1;
  }
  .timeline-wrap {
    padding: var(--s2) var(--s4) var(--s3);
    background: var(--bg);
  }
</style>
