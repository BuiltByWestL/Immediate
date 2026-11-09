<script lang="ts">
  import Camera from "@lucide/svelte/icons/camera";
  import Captions from "@lucide/svelte/icons/captions";
  import FlipHorizontal from "@lucide/svelte/icons/flip-horizontal-2";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Scissors from "@lucide/svelte/icons/scissors";
  import SquareSplitHorizontal from "@lucide/svelte/icons/square-split-horizontal";
  import X from "@lucide/svelte/icons/x";
  import Music from "@lucide/svelte/icons/music";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import Download from "@lucide/svelte/icons/download";
  import Image from "@lucide/svelte/icons/image";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { ui, updateSettings } from "../../lib/app.svelte";
  import Slider from "../image/Slider.svelte";
  import { grabFrame, saveSubtitles } from "./actions";
  import { blankEdit, changeMedia, commitMedia, cutSelection, fmtTime, GEQ_BANDS, keptSegments, media, pictureChanged, silenceSelection, snappedSegments, splitAt, totalLength, type Range2D } from "./session.svelte";

  let { isVideo, seek }: { isVideo: boolean; seek: (t: number) => void } = $props();

  const advanced = $derived(ui.settings.mode === "advanced");
  const kept = $derived(keptSegments());
  const sel = $derived(media.selection);

  function cutOut() {
    const to = cutSelection();
    if (to != null) seek(to);
  }

  const canSplit = $derived(kept.some((s) => media.time > s.start + 0.05 && media.time < s.end - 0.05));

  function keepSelection() {
    if (!sel) return;
    const s = { ...sel };
    changeMedia((e) => {
      e.trim = { start: s.start, end: s.end };
      e.cuts = [];
    });
    media.selection = null;
    seek(s.start);
  }

  function resetEdits() {
    changeMedia((e) => Object.assign(e, blankEdit(media.duration)));
    media.cropAspect = null;
  }

  // What Save will write, in plain words.
  const streams = $derived((media.probe?.streams ?? []) as any[]);
  const v = $derived(streams.find((s) => s.codec_type === "video" && s.disposition?.attached_pic !== 1));
  const audios = $derived(streams.filter((s) => s.codec_type === "audio"));
  const subs = $derived(streams.filter((s) => s.codec_type === "subtitle"));
  const NAMES: Record<string, string> = { h264: "H.264", hevc: "HEVC", av1: "AV1", vp9: "VP9", vp8: "VP8", prores: "ProRes", mpeg4: "MPEG-4", mpeg2video: "MPEG-2", aac: "AAC", mp3: "MP3", opus: "Opus", flac: "FLAC", vorbis: "Vorbis", ac3: "AC-3", eac3: "E-AC-3", alac: "ALAC" };
  const cname = (c: string) => NAMES[c] ?? (c?.startsWith("pcm_") ? "PCM" : c?.toUpperCase());
  const mbps = (b: any) => (Number(b) ? `${(Number(b) / 1e6).toFixed(1)} Mb/s` : "");
  const container = $derived((ui.file?.ext ?? "").toUpperCase());
  const chapters = $derived((media.probe?.chapters ?? []).length > 0);

  /** Saving redraws the picture (no lossless cuts): crop, speed, or turning outside MP4/MOV. */
  const redraws = $derived(
    isVideo && (media.edit.speed !== 1 || !!media.edit.crop || ((media.edit.rotate !== 0 || media.edit.flip) && !["mp4", "m4v", "mov"].includes(ui.file?.ext ?? ""))),
  );
  const resultLength = $derived((media.mode === "lossless" && isVideo && !redraws ? totalLength(snappedSegments(kept, media.keyframes)) : totalLength(kept)) / media.edit.speed);
  const summary = $derived.by(() => {
    if (media.mode === "smaller") return `H.264${audios.length && !media.edit.mute ? " · AAC" : ""} · MP4 · about ${media.sizeMb} MB`;
    const parts: string[] = [];
    if (v) parts.push(`${cname(v.codec_name)}${v.profile ? " " + v.profile : ""}${v.bit_rate ? " · " + mbps(v.bit_rate) : ""}`);
    if (audios.length && !media.edit.mute) parts.push(audios.length > 1 ? `${audios.length} audio tracks` : `${cname(audios[0].codec_name)} audio`);
    if (media.edit.mute) parts.push("no sound");
    parts.push(container);
    if (subs.length && media.mode === "lossless") parts.push(subs.length > 1 ? `${subs.length} subtitle tracks` : "subtitles");
    if (media.mode === "lossless") parts.push(chapters ? "chapters and details kept" : "details kept");
    return parts.join(" · ");
  });

  const SIZES = [8, 10, 25, 50, 100];
  const FADES = [0, 0.5, 1, 2, 3, 5];
  const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 4];

  // Picture. The crop is kept in the turned picture's coordinates, so turning or mirroring
  // moves it along with the picture.
  const turnCw = (c: Range2D): Range2D => ({ x: 1 - c.y - c.h, y: c.x, w: c.h, h: c.w });
  function turn(by: 90 | -90) {
    changeMedia((e) => {
      e.rotate = (e.rotate + by + 360) % 360;
      if (e.crop) e.crop = by === 90 ? turnCw(e.crop) : turnCw(turnCw(turnCw(e.crop)));
    });
    if (media.cropAspect) media.cropAspect = 1 / media.cropAspect;
  }
  function mirror() {
    changeMedia((e) => {
      e.flip = !e.flip;
      if (e.crop) e.crop = { ...e.crop, x: 1 - e.crop.x - e.crop.w };
    });
  }
  /** The picture's shape as shown (after the file's own rotation and the turn here). */
  const shown = $derived.by(() => {
    if (!v) return { w: 16, h: 9 };
    const matrix = (v.side_data_list ?? []).find((d: any) => d.rotation != null);
    const sourceCw = (((-(matrix?.rotation ?? 0)) % 360) + 360) % 360;
    const sideways = (sourceCw + media.edit.rotate) % 180 !== 0;
    return sideways ? { w: v.height, h: v.width } : { w: v.width, h: v.height };
  });
  const ASPECTS: { label: string; r: number | null }[] = [
    { label: "Free", r: null },
    { label: "16:9", r: 16 / 9 },
    { label: "9:16", r: 9 / 16 },
    { label: "1:1", r: 1 },
    { label: "4:5", r: 4 / 5 },
  ];
  function setCrop(r: number | null | "none") {
    if (r === "none") {
      media.cropAspect = null;
      changeMedia((e) => (e.crop = null));
      return;
    }
    media.cropAspect = r;
    // The largest centered crop of that shape (or the whole picture, to drag from).
    const frame = shown.w / shown.h;
    const w = r === null ? 1 : r > frame ? 1 : r / frame;
    const h = r === null ? 1 : r > frame ? frame / r : 1;
    changeMedia((e) => (e.crop = { x: (1 - w) / 2, y: (1 - h) / 2, w, h }));
  }
  const cropping = $derived(media.edit.crop !== null);

  // Tracks.
  const THUMB_TYPES = ["mp4", "m4v", "mov", "mkv"];
  const textSub = (s: any) => ["subrip", "srt", "ass", "ssa", "webvtt", "mov_text", "text"].includes(s.codec_name);
  function trackName(s: any, kind: string, i: number, count: number) {
    const bits = [count > 1 ? `${kind} ${i + 1}` : kind];
    const lang = s.tags?.language;
    if (s.tags?.title) bits.push(s.tags.title);
    else if (lang && lang !== "und") bits.push(lang.toUpperCase());
    return bits.join(" · ");
  }
  function toggleTrack(kind: "keepAudio" | "keepSubs", i: number) {
    changeMedia((e) => {
      const flags = [...e[kind]];
      flags[i] = !(flags[i] ?? true);
      e[kind] = flags;
    });
  }
  const pictureSubs = $derived(subs.some((s) => !["subrip", "srt", "ass", "ssa", "webvtt", "mov_text", "text"].includes(s.codec_name)));
  async function addSubtitles() {
    const picked = await openDialog({ multiple: false, directory: false, filters: [{ name: "Subtitles", extensions: ["srt", "vtt", "ass", "ssa"] }] });
    if (typeof picked === "string") changeMedia((e) => (e.subFile = picked));
  }
  const fileName = (p: string) => p.split(/[\\/]/).pop();

  // Audio files: tags and cover art. Tags are read from the file (any letter case) and only
  // the ones changed here are written.
  const TAGS = [
    { key: "title", label: "Title" },
    { key: "artist", label: "Artist" },
    { key: "album", label: "Album" },
    { key: "album_artist", label: "Album artist" },
    { key: "date", label: "Year" },
    { key: "track", label: "Track" },
    { key: "genre", label: "Genre" },
  ];
  const original = $derived.by(() => {
    const out: Record<string, string> = {};
    for (const src of [media.probe?.format?.tags, audios[0]?.tags]) for (const [k, val] of Object.entries(src ?? {})) out[k.toLowerCase()] ??= String(val);
    return out;
  });
  const tagValue = (k: string) => media.edit.tags[k] ?? original[k] ?? "";
  function setTag(k: string, val: string) {
    if (val === (original[k] ?? "")) delete media.edit.tags[k];
    else media.edit.tags[k] = val;
  }
  const ext = $derived(ui.file?.ext ?? "");
  const holdsCover = $derived(["mp3", "m4a", "m4b", "flac"].includes(ext));
  const coverShown = $derived(media.edit.cover === "" ? null : media.edit.cover ? convertFileSrc(media.edit.cover) : media.coverUrl);
  async function pickCover() {
    const picked = await openDialog({ multiple: false, directory: false, filters: [{ name: "Pictures", extensions: ["jpg", "jpeg", "png", "webp", "bmp"] }] });
    if (typeof picked === "string") changeMedia((e) => (e.cover = picked));
  }

  // Audio files: sample rate and bit depth.
  const srcRate = $derived(Number(audios[0]?.sample_rate) || 0);
  const srcBits = $derived(Number(audios[0]?.bits_per_raw_sample) || Number(audios[0]?.bits_per_sample) || (String(audios[0]?.codec_name).match(/(\d+)/)?.[1] ? Number(String(audios[0]?.codec_name).match(/(\d+)/)?.[1]) : 0));
  const RATES = [44100, 48000, 96000];
  const srcChannels = $derived(Number(audios[0]?.channels) || 0);

  // Equalizer.
  const EQ_PRESETS = [
    { name: "Flat", bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    { name: "Voice", bands: [-4, -3, -2, 0, 0, 1, 3, 3, 1, 0] },
    { name: "More bass", bands: [6, 5, 4, 2, 0, 0, 0, 0, 0, 0] },
    { name: "More treble", bands: [0, 0, 0, 0, 0, 0, 1, 3, 5, 6] },
    { name: "Fuller", bands: [5, 4, 2, 0, -1, -1, 0, 2, 4, 5] },
    { name: "Less hiss", bands: [0, 0, 0, 0, 0, 0, 0, -2, -6, -9] },
  ];
  const LOW_CUTS = [0, 40, 80, 120, 200];
  const HIGH_CUTS = [0, 16000, 12000, 8000, 3400];
  const bandName = (f: number) => (f >= 1000 ? `${f / 1000}k` : String(f));

  // Clipping: the loudest sample after the volume change goes over full scale.
  const clips = $derived(!media.edit.normalize && media.peak * Math.pow(10, media.edit.gain / 20) > 1.0);
  const deep = $derived(["wav", "flac", "aif", "aiff"].includes(ext));
</script>

<aside class="panel" aria-label="Edit">
  <div class="body">
    <section>
      <h3>Trim</h3>
      <p class="hint">Drag the handles at either end of the timeline, or press <kbd>Q</kbd> (start) and <kbd>E</kbd> (end) at the playhead.</p>
    </section>

    <section>
      <h3>Cut out a part</h3>
      <p class="hint">Drag across the part on the timeline, then press <kbd>W</kbd> or Cut out. Or press <kbd>W</kbd> at each end to split there, double-click the part between, and press <kbd>W</kbd> again.</p>
      <div class="row">
        {#if sel}
          <button type="button" class="btn" onclick={cutOut} title="Cut out (W or Delete)">
            <Scissors size={16} strokeWidth={1.5} />Cut out
          </button>
        {:else}
          <button type="button" class="btn" disabled={!canSplit} onclick={() => splitAt(media.time)} title="Split at the playhead (W)">
            <SquareSplitHorizontal size={16} strokeWidth={1.5} />Split
          </button>
        {/if}
        <button type="button" class="btn" disabled={!sel} onclick={keepSelection}>Keep only this</button>
        {#if !isVideo}
          <button type="button" class="btn" disabled={!sel} onclick={silenceSelection} title="Make the selected part silent, keeping its length"><VolumeX size={16} strokeWidth={1.5} />Silence</button>
        {/if}
      </div>
      {#if sel}<p class="hint num">Selected {fmtTime(sel.end - sel.start, true)}</p>{/if}
      {#if media.edit.silences.length}
        <p class="hint">
          {media.edit.silences.length === 1 ? "1 part is" : `${media.edit.silences.length} parts are`} silenced.
          <button type="button" class="link" onclick={() => changeMedia((e) => (e.silences = []))}>Bring the sound back</button>
        </p>
      {/if}
    </section>

    {#if audios.length}
      <section>
        <div class="toggle">
          <span>Remove sound</span>
          <button type="button" class="switch" role="switch" aria-checked={media.edit.mute} aria-label="Remove sound" onclick={() => changeMedia((e) => (e.mute = !e.mute))}></button>
        </div>
        {#if !media.edit.mute}
          {#if advanced || !isVideo}
            <Slider label="Volume (dB)" min={-20} max={20} bind:value={media.edit.gain} oncommit={commitMedia} />
            {#if clips}
              <p class="note warn">The loudest parts will be too loud and distort. Lower the volume a little, or turn on Even out loudness.</p>
            {/if}
          {/if}
          {#if advanced || !isVideo}
            <div class="fades">
              <label>
                <span>Fade in</span>
                <select class="field" value={String(media.edit.fadeIn)} onchange={(e) => changeMedia((ed) => (ed.fadeIn = Number(e.currentTarget.value)))}>
                  {#each FADES as f (f)}<option value={String(f)}>{f ? `${f} s` : "None"}</option>{/each}
                </select>
              </label>
              <label>
                <span>Fade out</span>
                <select class="field" value={String(media.edit.fadeOut)} onchange={(e) => changeMedia((ed) => (ed.fadeOut = Number(e.currentTarget.value)))}>
                  {#each FADES as f (f)}<option value={String(f)}>{f ? `${f} s` : "None"}</option>{/each}
                </select>
              </label>
            </div>
            <div class="toggle">
              <span>Even out loudness</span>
              <button type="button" class="switch" role="switch" aria-checked={media.edit.normalize} aria-label="Even out loudness" onclick={() => changeMedia((e) => (e.normalize = !e.normalize))}></button>
            </div>
            {#if advanced && media.edit.normalize}
              <div class="chips" role="radiogroup" aria-label="Even out by">
                <button type="button" class="chip" role="radio" aria-checked={!media.edit.normalizePeak} onclick={() => changeMedia((e) => (e.normalizePeak = false))}>By loudness</button>
                <button type="button" class="chip" role="radio" aria-checked={media.edit.normalizePeak} onclick={() => changeMedia((e) => (e.normalizePeak = true))}>By the loudest peak</button>
              </div>
              <p class="hint">
                {media.edit.normalizePeak
                  ? "Raises or lowers everything so the loudest moment sits just below the maximum (-1 dB). Nothing else changes."
                  : "Sets an even, comfortable level (-16 LUFS, common for speech and music online)."}
              </p>
            {/if}
            {#if advanced}
              <h3 class="sub">Tone</h3>
              <Slider label="Bass" min={-12} max={12} bind:value={media.edit.eq[0]} oncommit={commitMedia} />
              <Slider label="Mid" min={-12} max={12} bind:value={media.edit.eq[1]} oncommit={commitMedia} />
              <Slider label="Treble" min={-12} max={12} bind:value={media.edit.eq[2]} oncommit={commitMedia} />
              <h3 class="sub">Equalizer</h3>
              <div class="chips" role="radiogroup" aria-label="Equalizer preset">
                {#each EQ_PRESETS as p (p.name)}
                  <button type="button" class="chip" role="radio" aria-checked={p.bands.every((g, i) => g === media.edit.geq[i])} onclick={() => changeMedia((e) => (e.geq = [...p.bands]))}>{p.name}</button>
                {/each}
              </div>
              <div class="geq" role="group" aria-label="Equalizer bands">
                {#each GEQ_BANDS as f, i (f)}
                  <label class="band" title="{bandName(f)}: {media.edit.geq[i] > 0 ? '+' : ''}{media.edit.geq[i]} dB (double-click to reset)">
                    <span class="num gain">{media.edit.geq[i] > 0 ? "+" : ""}{media.edit.geq[i]}</span>
                    <input
                      type="range"
                      min="-12"
                      max="12"
                      step="1"
                      bind:value={media.edit.geq[i]}
                      onchange={commitMedia}
                      ondblclick={() => changeMedia((e) => (e.geq[i] = 0))}
                      aria-label="{bandName(f)} (dB)"
                    />
                    <span class="num">{bandName(f)}</span>
                  </label>
                {/each}
              </div>
              <div class="fades">
                <label>
                  <span>Low cut</span>
                  <select class="field" value={String(media.edit.lowCut)} onchange={(e) => changeMedia((ed) => (ed.lowCut = Number(e.currentTarget.value)))}>
                    {#each LOW_CUTS as f (f)}<option value={String(f)}>{f ? `Below ${f} Hz` : "Off"}</option>{/each}
                  </select>
                </label>
                <label>
                  <span>High cut</span>
                  <select class="field" value={String(media.edit.highCut)} onchange={(e) => changeMedia((ed) => (ed.highCut = Number(e.currentTarget.value)))}>
                    {#each HIGH_CUTS as f (f)}<option value={String(f)}>{f ? `Above ${bandName(f)}` : "Off"}</option>{/each}
                  </select>
                </label>
              </div>
              <p class="hint">Low cut removes rumble and hum; high cut removes hiss.</p>
              <div class="toggle">
                <span>Reduce background noise</span>
                <button type="button" class="switch" role="switch" aria-checked={media.edit.denoise} aria-label="Reduce background noise" onclick={() => changeMedia((e) => (e.denoise = !e.denoise))}></button>
              </div>
              {#if media.edit.denoise}<p class="hint">Heard in the saved file, not while playing here.</p>{/if}
              <div class="toggle">
                <span>Level out loud and quiet parts</span>
                <button type="button" class="switch" role="switch" aria-checked={media.edit.compress} aria-label="Level out loud and quiet parts" onclick={() => changeMedia((e) => (e.compress = !e.compress))}></button>
              </div>
            {/if}
            {#if isVideo && (media.edit.fadeIn || media.edit.fadeOut || media.edit.normalize || media.edit.eq.some((g) => g) || media.edit.geq.some((g) => g) || media.edit.lowCut || media.edit.highCut || media.edit.denoise || media.edit.compress)}
              <p class="note">Saving re-encodes the video to apply these sound changes, so it takes longer.</p>
            {/if}
          {/if}
        {/if}
      </section>
    {/if}

    {#if advanced && isVideo}
      <section>
        <h3>Picture</h3>
        <div class="row">
          <button type="button" class="btn" onclick={() => turn(-90)} title="Turn left"><RotateCcw size={16} strokeWidth={1.5} />Left</button>
          <button type="button" class="btn" onclick={() => turn(90)} title="Turn right"><RotateCw size={16} strokeWidth={1.5} />Right</button>
          <button type="button" class="btn" aria-pressed={media.edit.flip} onclick={mirror} title="Mirror left to right"><FlipHorizontal size={16} strokeWidth={1.5} />Mirror</button>
        </div>
        <h3 class="sub">Crop</h3>
        <div class="chips" role="radiogroup" aria-label="Crop">
          <button type="button" class="chip" role="radio" aria-checked={!cropping} onclick={() => setCrop("none")}>None</button>
          {#each ASPECTS as a (a.label)}
            <button type="button" class="chip" role="radio" aria-checked={cropping && media.cropAspect === a.r} onclick={() => setCrop(a.r)}>{a.label}</button>
          {/each}
        </div>
        {#if cropping}<p class="hint">Drag the frame or its corners on the video.</p>{/if}
        <h3 class="sub">Thumbnail</h3>
        {#if !THUMB_TYPES.includes(ui.file?.ext ?? "")}
          <p class="hint">MP4, MOV and MKV files can carry a thumbnail. Save As one of those to add it.</p>
        {/if}
        {#if media.edit.coverFrame != null}
          <div class="added">
            <Image size={16} strokeWidth={1.5} />
            <button type="button" class="link name num" title="Go to this frame" onclick={() => seek(media.edit.coverFrame ?? 0)}>Frame at {fmtTime(media.edit.coverFrame, true)}</button>
            <button type="button" class="icon-btn" aria-label="Don’t change the thumbnail" title="Remove" onclick={() => changeMedia((e) => (e.coverFrame = null))}><X size={15} strokeWidth={1.5} /></button>
          </div>
        {/if}
        <button type="button" class="btn" onclick={() => changeMedia((e) => (e.coverFrame = media.time))}>
          <Image size={16} strokeWidth={1.5} />{media.edit.coverFrame != null ? "Use this frame instead" : "Use this frame as the thumbnail"}
        </button>
        {#if redraws && pictureChanged()}
          <p class="note">Saving re-encodes the video to apply these picture changes{media.edit.rotate || media.edit.flip ? " (turning alone stays lossless in MP4 and MOV)" : ""}.</p>
        {/if}
      </section>
    {/if}

    {#if advanced}
      <section>
        <h3>Speed</h3>
        <div class="chips" role="radiogroup" aria-label="Speed">
          {#each SPEEDS as s (s)}
            <button type="button" class="chip num" role="radio" aria-checked={media.edit.speed === s} onclick={() => changeMedia((e) => (e.speed = s))}>{s}×</button>
          {/each}
        </div>
        {#if media.edit.speed !== 1 && audios.length && !media.edit.mute}
          <div class="toggle">
            <span>Keep the pitch</span>
            <button type="button" class="switch" role="switch" aria-checked={media.edit.keepPitch} aria-label="Keep the pitch" onclick={() => changeMedia((e) => (e.keepPitch = !e.keepPitch))}></button>
          </div>
          {#if !media.edit.keepPitch}<p class="hint">The pitch follows the speed, like a tape played faster or slower.</p>{/if}
        {/if}
        {#if !isVideo}
          <div class="toggle">
            <span>Play backwards</span>
            <button type="button" class="switch" role="switch" aria-checked={media.edit.reverse} aria-label="Play backwards" onclick={() => changeMedia((e) => (e.reverse = !e.reverse))}></button>
          </div>
          {#if media.edit.reverse}<p class="hint">Heard in the saved file; it plays forwards here so you can still find your place.</p>{/if}
        {/if}
      </section>
    {/if}

    {#if advanced && !isVideo}
      <section>
        <h3>Tags</h3>
        <div class="cover-row">
          <div class="cover">
            {#if coverShown}<img src={coverShown} alt="Cover art" />{:else}<Music size={22} strokeWidth={1.25} />{/if}
          </div>
          <div class="cover-actions">
            <button type="button" class="btn" disabled={!holdsCover} onclick={pickCover}>{coverShown ? "Change cover…" : "Add cover…"}</button>
            {#if coverShown}<button type="button" class="btn" onclick={() => changeMedia((e) => (e.cover = ""))}>Remove</button>{/if}
          </div>
        </div>
        {#if !holdsCover}<p class="hint">Cover art can be saved in MP3, M4A and FLAC files.</p>{/if}
        {#each TAGS as t (t.key)}
          <label class="tag">
            <span>{t.label}</span>
            <input class="field" value={tagValue(t.key)} oninput={(e) => setTag(t.key, e.currentTarget.value)} onchange={commitMedia} spellcheck="false" />
          </label>
        {/each}
      </section>

      <section>
        <h3>Format</h3>
        <label class="tag">
          <span>Sample rate</span>
          <select
            class="field"
            value={String(media.edit.sampleRate ?? "")}
            onchange={(e) => changeMedia((ed) => (ed.sampleRate = e.currentTarget.value ? Number(e.currentTarget.value) : null))}
          >
            <option value="">Same{srcRate ? ` (${srcRate / 1000} kHz)` : ""}</option>
            {#each RATES.filter((r) => r !== srcRate) as r (r)}<option value={String(r)}>{r / 1000} kHz</option>{/each}
          </select>
        </label>
        <label class="tag">
          <span>Channels</span>
          <select class="field" value={String(media.edit.channels ?? "")} onchange={(e) => changeMedia((ed) => (ed.channels = e.currentTarget.value ? (Number(e.currentTarget.value) as 1 | 2) : null))}>
            <option value="">Same{srcChannels === 1 ? " (mono)" : srcChannels === 2 ? " (stereo)" : ""}</option>
            {#if srcChannels !== 1}<option value="1">Mono</option>{/if}
            {#if srcChannels !== 2}<option value="2">Stereo</option>{/if}
          </select>
        </label>
        {#if deep}
          <label class="tag">
            <span>Bit depth</span>
            <select class="field" value={String(media.edit.bitDepth ?? "")} onchange={(e) => changeMedia((ed) => (ed.bitDepth = e.currentTarget.value ? Number(e.currentTarget.value) : null))}>
              <option value="">Same{srcBits ? ` (${srcBits}-bit)` : ""}</option>
              {#each [16, 24].filter((b) => b !== srcBits) as b (b)}<option value={String(b)}>{b}-bit</option>{/each}
            </select>
          </label>
        {/if}
        <p class="hint">To save as another format (WAV, FLAC, MP3, M4A, Opus, OGG, AIFF), use Save As, or pick one for every save in Settings → Audio.</p>
      </section>
    {/if}

    {#if advanced && isVideo}
      <section>
        <h3>Tracks</h3>
        {#each audios as a, i (i)}
          <div class="toggle">
            <span>{trackName(a, "Sound", i, audios.length)} <span class="dim">{cname(a.codec_name)}</span></span>
            <button type="button" class="switch" role="switch" aria-checked={media.edit.keepAudio[i] ?? true} aria-label="Keep {trackName(a, 'Sound', i, audios.length)}" onclick={() => toggleTrack("keepAudio", i)}></button>
          </div>
        {/each}
        {#each subs as s, i (i)}
          <div class="toggle">
            <span>{trackName(s, "Subtitles", i, subs.length)}</span>
            {#if textSub(s)}
              <button type="button" class="icon-btn" title="Save as an .srt file, timed to your edit" aria-label="Save {trackName(s, 'Subtitles', i, subs.length)} as a file" onclick={() => saveSubtitles(i)}><Download size={15} strokeWidth={1.5} /></button>
            {/if}
            <button type="button" class="switch" role="switch" aria-checked={media.edit.keepSubs[i] ?? true} aria-label="Keep {trackName(s, 'Subtitles', i, subs.length)}" onclick={() => toggleTrack("keepSubs", i)}></button>
          </div>
        {/each}
        {#if pictureSubs}<p class="hint">Picture-based subtitles stay only in MKV files saved Lossless.</p>{/if}
        {#if media.edit.subFile}
          <div class="added">
            <Captions size={16} strokeWidth={1.5} />
            <span class="name" title={media.edit.subFile}>{fileName(media.edit.subFile)}</span>
            <button type="button" class="icon-btn" aria-label="Remove the added subtitles" title="Remove" onclick={() => changeMedia((e) => (e.subFile = null))}><X size={15} strokeWidth={1.5} /></button>
          </div>
          <p class="hint">Added as a subtitle track, following your cuts.</p>
        {:else}
          <button type="button" class="btn" onclick={addSubtitles}><Captions size={16} strokeWidth={1.5} />Add subtitles…</button>
        {/if}
      </section>
    {/if}

    <section>
      <div class="toggle">
        <span>Make smaller</span>
        <button
          type="button"
          class="switch"
          role="switch"
          aria-checked={media.mode === "smaller"}
          aria-label="Make smaller"
          onclick={() => (media.mode = media.mode === "smaller" ? "lossless" : "smaller")}
        ></button>
      </div>
      {#if media.mode === "smaller"}
        <p class="hint">Re-encodes to fit a size limit, like the 10 MB limit for Discord uploads.</p>
        <div class="chips" role="radiogroup" aria-label="Target size">
          {#each SIZES as mb (mb)}
            <button type="button" class="chip" role="radio" aria-checked={media.sizeMb === mb} onclick={() => (media.sizeMb = mb)}>{mb} MB</button>
          {/each}
        </div>
      {/if}
    </section>

    {#if advanced}
      {#if media.mode !== "smaller"}
        <section>
          <h3>Cuts</h3>
          <div class="segmented" role="radiogroup" aria-label="How to save">
            <button type="button" role="radio" aria-checked={media.mode === "lossless"} onclick={() => (media.mode = "lossless")}>Lossless</button>
            <button type="button" role="radio" aria-checked={media.mode === "accurate"} onclick={() => (media.mode = "accurate")}>Frame-exact</button>
          </div>
          <p class="hint">
            {#if media.mode === "lossless"}
              Instant and untouched quality. Each kept part starts at the nearest keyframe before it (marked on the timeline).
            {:else}
              Cuts exactly where you set them. Re-encodes at the original’s codec and bitrate, which takes longer.
            {/if}
          </p>
        </section>
      {/if}
      {#if isVideo}
        <section>
          <button type="button" class="btn" onclick={grabFrame}><Camera size={16} strokeWidth={1.5} />Save this frame</button>
        </section>
      {/if}
    {/if}

    <section>
      <button type="button" class="btn" disabled={!media.past.length} onclick={resetEdits}>Start over</button>
    </section>
  </div>

  <footer>
    <p class="num"><span class="dim">Result</span> {fmtTime(resultLength, true)}</p>
    <p class="dim small">{summary}</p>
    {#if !advanced}
      <button type="button" class="link small" onclick={() => updateSettings({ mode: "advanced" })}>More options</button>
    {/if}
  </footer>
</aside>

<style>
  .panel {
    flex: none;
    width: 288px;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-left: 1px solid var(--line);
    min-height: 0;
  }
  .body {
    flex: 1;
    overflow-y: auto;
    padding: var(--s1) var(--s4) var(--s4);
  }
  section {
    padding: var(--s3) 0;
    border-bottom: 1px solid var(--line);
  }
  h3 {
    margin-bottom: var(--s1);
    font-size: var(--text-md);
    font-weight: var(--w-medium);
  }
  .hint {
    margin-bottom: var(--s2);
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  /* About the whole sound section, so it stands apart from the last switch above it. */
  .note {
    margin-top: var(--s3);
    padding-top: var(--s2);
    border-top: 1px solid var(--line);
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  .row {
    display: flex;
    gap: var(--s2);
  }
  .toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 28px;
    margin-bottom: var(--s1);
  }
  .sub {
    margin-top: var(--s3);
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  .fades {
    display: flex;
    gap: var(--s3);
    margin: var(--s2) 0;
  }
  .fades label {
    flex: 1;
    display: grid;
    gap: 2px;
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s1);
  }
  .chip {
    height: 26px;
    padding: 0 var(--s2);
    border-radius: 13px;
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--text);
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
    cursor: pointer;
  }
  .chip[aria-checked="true"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .btn[aria-pressed="true"] {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .row .btn {
    flex: 1;
    padding: 0 var(--s1);
  }
  .geq {
    display: grid;
    grid-template-columns: repeat(10, 1fr);
    gap: 2px;
    margin-top: var(--s2);
  }
  .band {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s1);
    font-size: 11px;
    color: var(--text-dim);
  }
  .band .gain {
    color: var(--text);
    min-height: 14px;
  }
  .band input {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 20px;
    height: 110px;
    margin: 0;
    accent-color: var(--accent);
  }
  .note.warn {
    color: var(--danger);
  }
  .added {
    display: flex;
    align-items: center;
    gap: var(--s2);
    min-height: 30px;
  }
  .added .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chips + .toggle {
    margin-top: var(--s2);
  }
  .cover-row {
    display: flex;
    gap: var(--s3);
    align-items: center;
    margin-bottom: var(--s2);
  }
  .cover {
    flex: none;
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--text-dim);
    overflow: hidden;
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .cover-actions {
    display: grid;
    gap: var(--s1);
  }
  .tag {
    display: grid;
    grid-template-columns: 96px 1fr;
    align-items: center;
    gap: var(--s2);
    margin-bottom: var(--s1);
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  .tag .field {
    min-width: 0;
    width: 100%;
  }
  footer {
    padding: var(--s3) var(--s4);
    border-top: 1px solid var(--line);
    background: var(--bg);
  }
  .small {
    font-size: var(--text-sm);
  }
  .link {
    margin-top: var(--s1);
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
</style>
