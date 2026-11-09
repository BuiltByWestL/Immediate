<script lang="ts">
  // Advanced → Save: the format, quality, size and details of the saved photo. Applies to Save
  // and Save As (Replace keeps the original's format).
  import { editor, session } from "./editor.svelte";
  import { outputSize } from "./geometry";
  import { FORMATS } from "./save";

  // session.info isn't reactive; it is set before editor.ready turns true.
  const info = $derived(editor.ready ? session.info : null);
  const src = $derived(session.pipeline?.src ?? null);
  const out = $derived(editor.output);
  const format = $derived(out.format ?? info?.saveFormat ?? "jpeg");
  const lossy = $derived(format === "jpeg" || (format === "webp" && !(out.format === null && info?.lossless)));

  const QUALITIES = [95, 90, 80, 70, 60];
  const SIDES = [3840, 2560, 1920, 1280, 800];
  const LIMITS = [
    [10_000_000, "10 MB"],
    [8_000_000, "8 MB"],
    [5_000_000, "5 MB"],
    [2_000_000, "2 MB"],
    [1_000_000, "1 MB"],
    [500_000, "500 KB"],
  ] as const;

  /** The saved size for a long side, from the current crop's shape. */
  function dims(long: number | null) {
    if (!src) return "";
    const g = outputSize(src, editor.edits.geometry);
    const k = long && long < Math.max(g.w, g.h) ? long / Math.max(g.w, g.h) : 1;
    return `${Math.round(g.w * k)} × ${Math.round(g.h * k)}`;
  }
</script>

{#if info?.hdr}
  <p class="note">This photo has HDR brightness information. Saving keeps the picture but not the extra brightness, so it looks like a normal photo on HDR screens.</p>
{/if}

<section>
  <h3>Format</h3>
  <div class="chips" role="radiogroup" aria-label="Format">
    <button type="button" class="chip" role="radio" aria-checked={out.format === null} onclick={() => (editor.output.format = null)}>
      Same{info ? ` (${info.saveFormat.toUpperCase()})` : ""}
    </button>
    {#each FORMATS.filter((f) => f.id !== info?.saveFormat) as f (f.id)}
      <button type="button" class="chip" role="radio" aria-checked={out.format === f.id} onclick={() => (editor.output.format = f.id)}>{f.label}</button>
    {/each}
  </div>
  {#if out.format && out.format !== info?.saveFormat}<p class="hint">Replace original keeps the original’s format; Save and Save As use this one.</p>{/if}
</section>

{#if lossy}
  <section>
    <h3>Quality</h3>
    <div class="chips" role="radiogroup" aria-label="Quality">
      <button type="button" class="chip" role="radio" aria-checked={out.quality === null} onclick={() => (editor.output.quality = null)}>
        Same{info?.jpegQuality ? ` (${info.jpegQuality})` : ""}
      </button>
      {#each QUALITIES as q (q)}
        <button type="button" class="chip num" role="radio" aria-checked={out.quality === q} onclick={() => (editor.output.quality = q)}>{q}</button>
      {/each}
    </div>
    <p class="hint">Higher keeps more detail; lower makes a smaller file.</p>
  </section>
{/if}

<section>
  <h3>Size</h3>
  <div class="chips" role="radiogroup" aria-label="Size">
    <button type="button" class="chip" role="radio" aria-checked={out.longSide === null} onclick={() => (editor.output.longSide = null)}>Full size</button>
    {#each SIDES as s (s)}
      <button type="button" class="chip num" role="radio" aria-checked={out.longSide === s} onclick={() => (editor.output.longSide = s)}>{s}</button>
    {/each}
  </div>
  <p class="hint num">Saves at {dims(out.longSide)} pixels{out.longSide ? ` (longest side ${out.longSide})` : ""}.</p>
</section>

{#if lossy}
  <section>
    <h3>Keep the file under</h3>
    <div class="chips" role="radiogroup" aria-label="Keep the file under">
      <button type="button" class="chip" role="radio" aria-checked={out.maxBytes === null} onclick={() => (editor.output.maxBytes = null)}>No limit</button>
      {#each LIMITS as [b, label] (b)}
        <button type="button" class="chip num" role="radio" aria-checked={out.maxBytes === b} onclick={() => (editor.output.maxBytes = b)}>{label}</button>
      {/each}
    </div>
    <p class="hint">For upload limits: lowers the quality as little as needed, and the size only if that isn’t enough.</p>
  </section>
{/if}

<section>
  <div class="toggle">
    <span>Remove all details</span>
    <button type="button" class="switch" role="switch" aria-checked={out.stripAll} aria-label="Remove all details" onclick={() => (editor.output.stripAll = !editor.output.stripAll)}></button>
  </div>
  <p class="hint">
    {out.stripAll
      ? "The saved photo carries no camera, date, location or editing details. Its colors stay the same."
      : "The saved photo keeps the original’s details (camera, date, and location unless Settings removes it)."}
  </p>
</section>

<style>
  section {
    padding: var(--s3) 0;
    border-top: 1px solid var(--line);
  }
  section:first-of-type {
    border-top: 0;
  }
  h3 {
    margin-bottom: var(--s2);
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
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
    border: 1px solid var(--line);
    border-radius: 13px;
    background: transparent;
    color: var(--text);
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .chip[aria-checked="true"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .hint,
  .note {
    margin-top: var(--s2);
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  .note {
    margin: 0 0 var(--s2);
  }
  .toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s2);
  }
</style>
