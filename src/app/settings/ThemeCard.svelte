<script lang="ts">
  // A live miniature of the app drawn with the theme's own tokens.
  import { themeStyle, type Theme } from "../../design/theme";

  let {
    themes,
    name,
    selected,
    onselect,
  }: { themes: Theme[]; name: string; selected: boolean; onselect: () => void } = $props();
</script>

<button type="button" class="card" role="radio" aria-checked={selected} onclick={onselect}>
  <span class="mini">
    {#each themes as t (t.id)}
      <span class="half" style={themeStyle(t.tokens)}>
        <span class="bar"></span>
        <span class="body">
          <span class="canvas"></span>
          <span class="side">
            <span class="l1"></span>
            <span class="l2"></span>
            <span class="pill"></span>
          </span>
        </span>
      </span>
    {/each}
  </span>
  <span class="name">{name}</span>
</button>

<style>
  .card {
    display: grid;
    gap: var(--s2);
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .mini {
    display: flex;
    height: 72px;
    border-radius: var(--radius);
    overflow: hidden;
    box-shadow: 0 0 0 1px var(--line);
  }
  .card[aria-checked="true"] .mini {
    box-shadow: 0 0 0 2px var(--focus);
  }
  .card:focus-visible {
    outline: none;
  }
  .card:focus-visible .mini {
    outline: 2px solid var(--focus);
    outline-offset: 3px;
  }
  .half {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .bar {
    height: 12px;
    border-bottom: 1px solid var(--line);
  }
  .body {
    flex: 1;
    display: flex;
  }
  .canvas {
    flex: 1;
    background: var(--canvas);
  }
  .side {
    width: 38%;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 6px;
    background: var(--panel);
    border-left: 1px solid var(--line);
  }
  .l1,
  .l2 {
    height: 4px;
    border-radius: 2px;
    background: var(--text);
  }
  .l2 {
    width: 70%;
    background: var(--text-dim);
  }
  .pill {
    margin-top: auto;
    height: 9px;
    border-radius: 3px;
    background: var(--accent);
  }
  .name {
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
  }
</style>
