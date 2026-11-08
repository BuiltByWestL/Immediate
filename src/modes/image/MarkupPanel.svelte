<script lang="ts">
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import Circle from "@lucide/svelte/icons/circle";
  import Eraser from "@lucide/svelte/icons/eraser";
  import Grid from "@lucide/svelte/icons/grid-2x2";
  import Highlighter from "@lucide/svelte/icons/highlighter";
  import Minus from "@lucide/svelte/icons/minus";
  import Pen from "@lucide/svelte/icons/pen-line";
  import Square from "@lucide/svelte/icons/square";
  import Hash from "@lucide/svelte/icons/hash";
  import Type from "@lucide/svelte/icons/type";
  import { ui } from "../../lib/app.svelte";
  import { change, editor } from "./editor.svelte";
  import { COLORS, type ToolId } from "./markup";

  const TOOLS: { id: ToolId; label: string; icon: typeof Pen; simple?: boolean; hint: string }[] = [
    { id: "pen", label: "Pen", icon: Pen, simple: true, hint: "Draw freely." },
    { id: "highlighter", label: "Highlight", icon: Highlighter, simple: true, hint: "Mark text or areas, see-through." },
    { id: "arrow", label: "Arrow", icon: ArrowUpRight, simple: true, hint: "Drag to point at something. Hold Shift for straight angles." },
    { id: "text", label: "Text", icon: Type, simple: true, hint: "Click where the text goes, type, press Enter." },
    { id: "blur", label: "Hide", icon: Grid, simple: true, hint: "Drag over faces, names or numbers to pixelate them for good." },
    { id: "line", label: "Line", icon: Minus, hint: "Drag. Hold Shift for straight angles." },
    { id: "rect", label: "Box", icon: Square, hint: "Drag. Hold Shift for a square." },
    { id: "ellipse", label: "Circle", icon: Circle, hint: "Drag. Hold Shift for a perfect circle." },
    { id: "number", label: "Number", icon: Hash, hint: "Click to place numbered steps: 1, 2, 3…" },
    { id: "eraser", label: "Eraser", icon: Eraser, hint: "Click a mark to remove it." },
  ];

  /** Tools with a Size control: the drawing tools, plus text. */
  const SIZED = new Set<ToolId>(["pen", "highlighter", "arrow", "line", "rect", "ellipse", "text"]);

  const advanced = $derived(ui.settings.mode === "advanced");
  const tools = $derived(TOOLS.filter((t) => advanced || t.simple));
  const current = $derived(TOOLS.find((t) => t.id === editor.tool));
  const usesColor = $derived(editor.tool !== "blur" && editor.tool !== "eraser");

  $effect(() => {
    // A tool hidden by switching to Simple falls back to the pen.
    if (!tools.some((t) => t.id === editor.tool)) editor.tool = "pen";
  });
</script>

<section>
  <div class="tools" role="radiogroup" aria-label="Markup tool">
    {#each tools as t (t.id)}
      <button type="button" class="tool" role="radio" aria-checked={editor.tool === t.id} onclick={() => (editor.tool = t.id)}>
        <t.icon size={18} strokeWidth={1.5} />
        <span>{t.label}</span>
      </button>
    {/each}
  </div>
  {#if current}<p class="hint">{current.hint}</p>{/if}
</section>

{#if usesColor}
  <section>
    <h3>Color</h3>
    <div class="colors" role="radiogroup" aria-label="Color">
      {#each COLORS as c (c)}
        <button type="button" class="swatch" role="radio" aria-checked={editor.color === c} aria-label={c} style:--c={c} onclick={() => (editor.color = c)}></button>
      {/each}
    </div>
  </section>
{/if}

{#if SIZED.has(editor.tool)}
  <section>
    <h3>{editor.tool === "text" ? "Text size" : "Size"}</h3>
    <div class="segmented" role="radiogroup" aria-label="Size">
      {#each [["s", "Small"], ["m", "Medium"], ["l", "Large"]] as [id, label] (id)}
        <button type="button" role="radio" aria-checked={editor.size === id} onclick={() => (editor.size = id as "s" | "m" | "l")}>{label}</button>
      {/each}
    </div>
  </section>
{/if}

<section>
  <button type="button" class="btn" disabled={!editor.edits.markup.length} onclick={() => change((e) => (e.markup = []))}>Remove all marks</button>
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
  .tools {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: var(--s1);
  }
  .tool {
    display: grid;
    justify-items: center;
    gap: 2px;
    padding: var(--s2) 0 var(--s1);
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text);
    font-size: 11px;
    font-weight: var(--w-medium);
    cursor: pointer;
  }
  .tool:hover {
    background: var(--hover);
  }
  .tool[aria-checked="true"] {
    background: var(--accent);
    color: var(--on-accent);
  }
  .hint {
    margin-top: var(--s2);
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  .colors {
    display: flex;
    gap: var(--s2);
  }
  .swatch {
    width: 24px;
    height: 24px;
    padding: 0;
    border-radius: 50%;
    border: 0;
    background: var(--c);
    box-shadow: inset 0 0 0 1px rgb(127 127 127 / 0.5);
    cursor: pointer;
  }
  .swatch[aria-checked="true"] {
    box-shadow: inset 0 0 0 1px rgb(127 127 127 / 0.5), 0 0 0 2px var(--panel), 0 0 0 4px var(--focus);
  }
</style>
