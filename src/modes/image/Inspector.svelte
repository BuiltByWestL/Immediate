<script lang="ts">
  import Wand from "@lucide/svelte/icons/wand-sparkles";
  import { ui, updateSettings } from "../../lib/app.svelte";
  import { autoEnhance, hiddenEditCount, SECTIONS } from "./edits";
  import { change, commit, editor, session, type Tab } from "./editor.svelte";
  import CropPanel from "./CropPanel.svelte";
  import FiltersPanel from "./FiltersPanel.svelte";
  import Slider from "./Slider.svelte";

  const TABS: [Tab, string][] = [
    ["adjust", "Adjust"],
    ["filters", "Filters"],
    ["crop", "Crop"],
  ];

  const advanced = $derived(ui.settings.mode === "advanced");
  const sections = $derived(
    SECTIONS.map((s) => ({ ...s, sliders: s.sliders.filter((d) => advanced || d.simple) })).filter((s) => s.sliders.length),
  );
  const hidden = $derived(advanced ? 0 : hiddenEditCount(editor.edits));

  function toggleAuto() {
    if (editor.edits.auto) {
      change((e) => (e.auto = null));
      return;
    }
    const p = session.pipeline;
    if (!p) return;
    // A small render of the untouched photo is enough to read its histogram.
    const k = Math.min(1, 256 / Math.max(p.src.w, p.src.h));
    const neutral = { ...editor.edits, adjust: { ...editor.edits.adjust }, auto: null, filter: { id: null, amount: 100 } };
    for (const key of Object.keys(neutral.adjust) as (keyof typeof neutral.adjust)[]) neutral.adjust[key] = 0;
    const { data } = p.renderFull(neutral, k);
    const auto = autoEnhance(data);
    change((e) => (e.auto = auto));
  }
</script>

<aside class="inspector" aria-label="Edit">
  <div class="tabs segmented" role="tablist">
    {#each TABS as [id, label] (id)}
      <button type="button" role="tab" aria-selected={editor.tab === id} onclick={() => (editor.tab = id)}>
        {label}
      </button>
    {/each}
  </div>

  <div class="body">
    {#if editor.tab === "adjust"}
      <div class="top">
        <button type="button" class="btn auto" aria-pressed={!!editor.edits.auto} disabled={!editor.ready} onclick={toggleAuto} title="Auto-enhance">
          <Wand size={16} strokeWidth={1.5} />Auto
        </button>
        <div class="segmented mode" role="radiogroup" aria-label="Tools shown">
          <button type="button" role="radio" aria-checked={!advanced} onclick={() => updateSettings({ mode: "simple" })}>Simple</button>
          <button type="button" role="radio" aria-checked={advanced} onclick={() => updateSettings({ mode: "advanced" })}>Advanced</button>
        </div>
      </div>
      {#if hidden}
        <p class="note">
          {hidden === 1 ? "1 edit" : `${hidden} edits`} from Advanced {hidden === 1 ? "is" : "are"} still applied.
          <button type="button" class="link" onclick={() => updateSettings({ mode: "advanced" })}>Show</button>
        </p>
      {/if}

      {#each sections as s (s.title)}
        <section>
          <h3>{s.title}</h3>
          {#each s.sliders as d (d.key)}
            <Slider label={d.label} min={d.min} max={d.max} bind:value={editor.edits.adjust[d.key]} oncommit={commit} />
          {/each}
        </section>
      {/each}
    {:else if editor.tab === "filters"}
      <FiltersPanel />
    {:else if editor.tab === "crop"}
      <CropPanel />
    {/if}
  </div>
</aside>

<style>
  .inspector {
    flex: none;
    width: 288px;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-left: 1px solid var(--line);
    min-height: 0;
  }
  .tabs {
    display: flex;
    margin: var(--s3);
  }
  .tabs button {
    flex: 1;
  }
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 0 var(--s4) var(--s6);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s2);
    padding: var(--s1) 0 var(--s3);
  }
  .auto[aria-pressed="true"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .mode button {
    padding: 0 var(--s2);
  }
  .note {
    margin-bottom: var(--s2);
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  section {
    padding: var(--s3) 0;
    border-top: 1px solid var(--line);
  }
  h3 {
    margin-bottom: var(--s1);
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
    color: var(--text-dim);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
    font-size: inherit;
  }
</style>
