<script lang="ts">
  import FlipHorizontal from "@lucide/svelte/icons/triangles-centerline-dashed-vertical";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw-square";
  import RectangleHorizontal from "@lucide/svelte/icons/rectangle-horizontal";
  import RectangleVertical from "@lucide/svelte/icons/rectangle-vertical";
  import { ui } from "../../lib/app.svelte";
  import { geometryChanged, NEUTRAL_GEOMETRY } from "./edits";
  import { change, commit, editor, session } from "./editor.svelte";
  import { effectiveCrop, fitAspect, flipHorizontal, rotateLeft, rotatedSize } from "./geometry";
  import Slider from "./Slider.svelte";

  const advanced = $derived(ui.settings.mode === "advanced");
  const g = $derived(editor.edits.geometry);
  const src = () => session.pipeline?.src ?? { w: 1, h: 1 };

  const PRESETS: [string, number | null][] = [
    ["Free", null],
    ["Original", -1],
    ["Square", 1],
    ["4:3", 4 / 3],
    ["3:2", 3 / 2],
    ["16:9", 16 / 9],
  ];

  const rot = $derived(rotatedSize(src(), g));
  const originalRatio = $derived(rot.w / rot.h);
  // Landscape or portrait for the chosen ratio (follows the photo's shape by default).
  let portrait = $state(false);
  $effect(() => {
    portrait = rot.h > rot.w;
  });

  function ratioFor(v: number | null): number | null {
    if (v === null) return null;
    if (v === -1) return originalRatio;
    if (v === 1) return 1;
    return portrait ? 1 / v : v;
  }

  function selected(v: number | null): boolean {
    const r = ratioFor(v);
    if (r === null || g.aspect === null) return r === g.aspect;
    return Math.abs(r - g.aspect) < 1e-3;
  }

  function setAspect(v: number | null) {
    const ratio = ratioFor(v);
    change((e) => {
      e.geometry.aspect = ratio;
      if (ratio) e.geometry.crop = fitAspect(effectiveCrop(e.geometry, src()), ratio, rotatedSize(src(), e.geometry));
    });
  }

  function flipOrientation() {
    portrait = !portrait;
    if (g.aspect && Math.abs(g.aspect - 1) > 1e-3) {
      const next = 1 / g.aspect;
      change((e) => {
        e.geometry.aspect = next;
        e.geometry.crop = fitAspect({ x: 0, y: 0, w: 1, h: 1 }, next, rotatedSize(src(), e.geometry));
      });
    }
  }
</script>

<section>
  <h3>Aspect ratio</h3>
  <div class="chips" role="radiogroup" aria-label="Aspect ratio">
    {#each PRESETS as [label, v] (label)}
      <button type="button" class="chip" role="radio" aria-checked={selected(v)} onclick={() => setAspect(v)}>{label}</button>
    {/each}
    <button
      type="button"
      class="icon-btn orient"
      title={portrait ? "Portrait (tall)" : "Landscape (wide)"}
      aria-label={portrait ? "Switch to landscape" : "Switch to portrait"}
      onclick={flipOrientation}
    >
      {#if portrait}<RectangleVertical size={16} strokeWidth={1.5} />{:else}<RectangleHorizontal size={16} strokeWidth={1.5} />{/if}
    </button>
  </div>
</section>

<section>
  <Slider label="Straighten" min={-45} max={45} bind:value={editor.edits.geometry.angle} oncommit={commit} />
  <div class="row">
    <button type="button" class="btn" onclick={() => change((e) => (e.geometry = rotateLeft(e.geometry)))} title="Turn left 90°">
      <RotateCcw size={16} strokeWidth={1.5} />Rotate
    </button>
    <button type="button" class="btn" onclick={() => change((e) => (e.geometry = flipHorizontal(e.geometry)))} title="Mirror left to right">
      <FlipHorizontal size={16} strokeWidth={1.5} />Flip
    </button>
  </div>
</section>

{#if advanced}
  <section>
    <h3>Perspective</h3>
    <Slider label="Vertical" bind:value={editor.edits.geometry.persV} oncommit={commit} />
    <Slider label="Horizontal" bind:value={editor.edits.geometry.persH} oncommit={commit} />
  </section>
{/if}

<section>
  <button type="button" class="btn" disabled={!geometryChanged(g)} onclick={() => change((e) => (e.geometry = structuredClone(NEUTRAL_GEOMETRY)))}>
    Reset crop and rotation
  </button>
</section>

<style>
  section {
    padding: var(--s3) 0;
    border-top: 1px solid var(--line);
  }
  section:first-child {
    border-top: 0;
    padding-top: var(--s1);
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
    align-items: center;
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
  .orient {
    width: 26px;
    height: 26px;
  }
  .row {
    display: flex;
    gap: var(--s2);
    margin-top: var(--s3);
  }
</style>
