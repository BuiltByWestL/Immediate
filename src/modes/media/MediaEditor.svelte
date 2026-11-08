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
  import CropFrame from "./CropFrame.svelte";
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
  // Audio nodes in the same order as the filters used when saving (export.rs): volume → tone →
  // leveling → loudness → fades. Only noise reduction is save-only.
  type LiveAudio = { ctx: AudioContext; gain: GainNode; eq: BiquadFilterNode[]; comp: DynamicsCompressorNode; norm: GainNode; limiter: DynamicsCompressorNode; fade: GainNode };
  let audio = $state.raw<LiveAudio | null>(null);

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
    // Fades are measured on the result, which plays at the chosen speed.
    const speed = media.edit.speed;
    pos /= speed;
    const total = totalLength(kept) / speed;
    let g = 1;
    if (fadeIn) g *= Math.min(1, pos / Math.min(fadeIn, total));
    if (fadeOut) g *= Math.min(1, (total - pos) / Math.min(fadeOut, total));
    return Math.max(0, g);
  }

  // "Even out loudness": the save measures the kept parts after volume, tone and leveling, then
  // applies one gain (plus a peak limiter when that gain would clip). Ask for the same whenever
  // those settings change (after a short pause, so dragging a slider doesn't start a measurement
  // per step); the last answer plays meanwhile.
  let norm = $state<{ db: number; limit: boolean }>({ db: 0, limit: false });
  let measureReq = 0;
  $effect(() => {
    const e = media.edit;
    const args = { path: file.path, segments: kept.map((s) => ({ start: s.start, end: s.end })), gainDb: e.gain, eq: [...e.eq], denoise: e.denoise, compress: e.compress };
    if (!e.normalize) return;
    const req = ++measureReq;
    const t = setTimeout(() => {
      invoke<[number, boolean]>("media_loudness", args)
        .then(([db, limit]) => req === measureReq && (norm = { db, limit }))
        .catch(() => {});
    }, 400);
    return () => clearTimeout(t);
  });
  const liveNorm = $derived(media.edit.normalize ? norm : { db: 0, limit: false });
  function connectAudio() {
    if (!el || audio) return;
    try {
      const ctx = new AudioContext();
      const src = ctx.createMediaElementSource(el);
      const gain = ctx.createGain();
      const norm = ctx.createGain();
      const fade = ctx.createGain();
      const mk = (type: BiquadFilterType, f: number) => {
        const b = ctx.createBiquadFilter();
        b.type = type;
        b.frequency.value = f;
        b.Q.value = 0.7;
        return b;
      };
      // Same bands as saving (export.rs): where laptop and phone speakers can play them.
      const eq = [mk("lowshelf", 200), mk("peaking", 1000), mk("highshelf", 4000)];
      const comp = ctx.createDynamicsCompressor();
      comp.threshold.value = -20;
      comp.ratio.value = 3;
      comp.attack.value = 0.015;
      comp.release.value = 0.25;
      // The saved file's alimiter at -1.5 dB: hard and fast, only peaks reach it.
      const limiter = ctx.createDynamicsCompressor();
      limiter.threshold.value = -1.5;
      limiter.knee.value = 0;
      limiter.ratio.value = 20;
      limiter.attack.value = 0.001;
      limiter.release.value = 0.05;
      src.connect(gain).connect(eq[0]).connect(eq[1]).connect(eq[2]);
      fade.connect(ctx.destination);
      gain.gain.value = Math.pow(10, media.edit.gain / 20);
      norm.gain.value = Math.pow(10, liveNorm.db / 20);
      fade.gain.value = fadeAt(el.currentTime);
      audio = { ctx, gain, eq, comp, norm, limiter, fade };
      route(media.edit.compress, liveNorm.limit);
    } catch {
      /* plays without live effects */
    }
  }
  /** Tone → (leveling) → loudness gain → (limiter) → fades; rewired only when a switch changes. */
  let routed = "";
  function route(compress: boolean, limit: boolean) {
    const a = audio;
    const key = `${compress}${limit}`;
    if (!a || routed === key) return;
    routed = key;
    for (const n of [a.eq[2], a.comp, a.norm, a.limiter]) n.disconnect();
    (compress ? a.eq[2].connect(a.comp) : a.eq[2]).connect(a.norm);
    (limit ? a.norm.connect(a.limiter) : a.norm).connect(a.fade);
  }
  // Follow every sound setting while playing. Read them all before checking `audio`, so the
  // effect subscribes to each one (an early return would leave it deaf to slider moves).
  $effect(() => {
    const gain = Math.pow(10, media.edit.gain / 20);
    const n = Math.pow(10, liveNorm.db / 20);
    const limit = liveNorm.limit;
    const eq = [...media.edit.eq];
    const compress = media.edit.compress;
    const a = audio;
    if (!a) return;
    const now = a.ctx.currentTime;
    a.gain.gain.setTargetAtTime(gain, now, 0.02);
    a.norm.gain.setTargetAtTime(n, now, 0.05);
    eq.forEach((g, i) => a.eq[i].gain.setTargetAtTime(g, now, 0.02));
    route(compress, limit);
  });
  $effect(() => () => audio?.ctx.close());

  function onerror() {
    if (!proxied) usePreview();
    else {
      stills = true;
      showStill(media.time);
    }
  }
  // Speed is heard and seen while playing; with "Keep the pitch" off the pitch follows it.
  $effect(() => {
    const speed = media.edit.speed;
    const keep = media.edit.keepPitch;
    if (!el) return;
    el.playbackRate = speed;
    el.preservesPitch = keep;
  });

  // The picture as it will be saved: turned and mirrored here, with the crop drawn over it.
  let stageW = $state(0);
  let stageH = $state(0);
  let natural = $state<{ w: number; h: number } | null>(null);
  const box = $derived.by(() => {
    if (!natural || !stageW || !stageH) return null;
    const sideways = media.edit.rotate % 180 !== 0;
    const w = sideways ? natural.h : natural.w;
    const h = sideways ? natural.w : natural.h;
    const s = Math.min(stageW / w, stageH / h);
    return { w: w * s, h: h * s, scale: s };
  });
  const pictureStyle = $derived.by(() => {
    if (!box || !natural) return "";
    const flip = media.edit.flip ? " scaleX(-1)" : "";
    return `width:${natural.w * box.scale}px;height:${natural.h * box.scale}px;inset:auto;left:50%;top:50%;object-fit:fill;transform:translate(-50%,-50%)${flip} rotate(${media.edit.rotate}deg)`;
  });

  function onloadedmetadata() {
    if (el instanceof HTMLVideoElement && el.videoWidth) natural = { w: el.videoWidth, h: el.videoHeight };
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
    <div class="stage" bind:clientWidth={stageW} bind:clientHeight={stageH}>
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
          style={pictureStyle}
        ></video>
      {:else if isVideo && stills}
        {#if still}<img
            src={still}
            alt=""
            style={pictureStyle}
            onload={(e) => {
              const i = e.currentTarget as HTMLImageElement;
              if (!natural && i.naturalWidth) natural = { w: i.naturalWidth, h: i.naturalHeight };
            }}
          />{/if}
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
      {#if isVideo && box && media.edit.crop}
        <CropFrame width={box.w} height={box.h} />
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
