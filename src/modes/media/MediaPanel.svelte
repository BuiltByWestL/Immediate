<script lang="ts">
  import Camera from "@lucide/svelte/icons/camera";
  import Scissors from "@lucide/svelte/icons/scissors";
  import { ui, updateSettings } from "../../lib/app.svelte";
  import Slider from "../image/Slider.svelte";
  import { grabFrame } from "./actions";
  import { changeMedia, commitMedia, fmtTime, keptSegments, media, snappedSegments, totalLength } from "./session.svelte";

  let { isVideo, seek }: { isVideo: boolean; seek: (t: number) => void } = $props();

  const advanced = $derived(ui.settings.mode === "advanced");
  const kept = $derived(keptSegments());
  const sel = $derived(media.selection);

  function cutSelection() {
    if (!sel) return;
    const s = { ...sel };
    changeMedia((e) => e.cuts.push(s));
    media.selection = null;
    seek(s.end);
  }

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
    changeMedia((e) => {
      e.trim = { start: 0, end: media.duration };
      e.cuts = [];
      e.mute = false;
      e.gain = 0;
    });
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

  const resultLength = $derived(media.mode === "lossless" && isVideo ? totalLength(snappedSegments(kept, media.keyframes)) : totalLength(kept));
  const summary = $derived.by(() => {
    if (media.mode === "smaller") return `H.264${audios.length && !media.edit.mute ? " · AAC" : ""} · MP4 · about ${media.sizeMb} MB`;
    const parts: string[] = [];
    if (v) parts.push(`${cname(v.codec_name)}${v.profile ? " " + v.profile : ""}${v.bit_rate ? " · " + mbps(v.bit_rate) : ""}`);
    if (audios.length && !media.edit.mute) parts.push(audios.length > 1 ? `${audios.length} audio tracks` : `${cname(audios[0].codec_name)} audio`);
    if (media.edit.mute) parts.push("no sound");
    parts.push(container);
    if (subs.length && media.mode === "lossless") parts.push(subs.length > 1 ? `${subs.length} subtitle tracks` : "subtitles");
    return parts.join(" · ");
  });

  const SIZES = [8, 10, 25, 50, 100];
</script>

<aside class="panel" aria-label="Edit">
  <div class="body">
    <section>
      <h3>Trim</h3>
      <p class="hint">Drag the handles at either end of the timeline, or press <kbd>I</kbd> and <kbd>O</kbd> at the playhead.</p>
    </section>

    <section>
      <h3>Cut out a part</h3>
      <p class="hint">Drag across the part on the timeline, then cut it out.</p>
      <div class="row">
        <button type="button" class="btn" disabled={!sel} onclick={cutSelection} title="Delete">
          <Scissors size={16} strokeWidth={1.5} />Cut out
        </button>
        <button type="button" class="btn" disabled={!sel} onclick={keepSelection}>Keep only this</button>
      </div>
      {#if sel}<p class="hint num">Selected {fmtTime(sel.end - sel.start, true)}</p>{/if}
    </section>

    {#if audios.length}
      <section>
        <div class="toggle">
          <span>Remove sound</span>
          <button type="button" class="switch" role="switch" aria-checked={media.edit.mute} aria-label="Remove sound" onclick={() => changeMedia((e) => (e.mute = !e.mute))}></button>
        </div>
        {#if advanced && !media.edit.mute}
          <Slider label="Volume (dB)" min={-20} max={20} bind:value={media.edit.gain} oncommit={commitMedia} />
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
