<script lang="ts">
  // A labelled slider: value shown on the right, fill from the zero point, double-click (or
  // Delete) resets. Arrow keys step by 1, Shift+arrows by 10. `oncommit` fires when a change ends.
  let {
    label,
    value = $bindable(),
    min = -100,
    max = 100,
    oncommit,
  }: { label: string; value: number; min?: number; max?: number; oncommit: () => void } = $props();

  let track: HTMLDivElement;
  let dragging = $state(false);

  const zero = $derived(Math.max(min, Math.min(max, 0)));
  const pct = (v: number) => ((v - min) / (max - min)) * 100;
  const fillFrom = $derived(Math.min(pct(zero), pct(value)));
  const fillTo = $derived(Math.max(pct(zero), pct(value)));
  const shown = $derived(Math.round(value));

  function setFromPointer(e: PointerEvent) {
    const r = track.getBoundingClientRect();
    const t = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
    let v = Math.round(min + t * (max - min));
    // A gentle snap to zero makes "back to untouched" easy to hit.
    if (Math.abs(v - zero) <= (max - min) * 0.012) v = zero;
    value = v;
  }

  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0) return;
    dragging = true;
    track.setPointerCapture(e.pointerId);
    setFromPointer(e);
  }
  function onpointermove(e: PointerEvent) {
    if (dragging) setFromPointer(e);
  }
  function onpointerup() {
    if (!dragging) return;
    dragging = false;
    oncommit();
  }
  function reset() {
    value = zero;
    oncommit();
  }
  function onkeydown(e: KeyboardEvent) {
    const step = e.shiftKey ? 10 : 1;
    let v = value;
    if (e.key === "ArrowRight" || e.key === "ArrowUp") v += step;
    else if (e.key === "ArrowLeft" || e.key === "ArrowDown") v -= step;
    else if (e.key === "Home") v = min;
    else if (e.key === "End") v = max;
    else if (e.key === "Delete" || e.key === "Backspace") v = zero;
    else return;
    e.preventDefault();
    e.stopPropagation();
    value = Math.max(min, Math.min(max, v));
    oncommit();
  }
</script>

<div class="slider" class:changed={shown !== zero}>
  <div class="head">
    <span class="label">{label}</span>
    <span class="value num">{shown > 0 && min < 0 ? "+" : ""}{shown}</span>
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="track"
    class:dragging
    bind:this={track}
    role="slider"
    tabindex="0"
    aria-label={label}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={shown}
    title="Double-click to reset"
    {onpointerdown}
    {onpointermove}
    {onpointerup}
    onpointercancel={onpointerup}
    ondblclick={reset}
    {onkeydown}
  >
    <div class="rail"></div>
    {#if min < 0}<div class="tick" style:left="{pct(zero)}%"></div>{/if}
    <div class="fill" style:left="{fillFrom}%" style:width="{fillTo - fillFrom}%"></div>
    <div class="thumb" style:left="{pct(value)}%"></div>
  </div>
</div>

<style>
  .slider {
    padding: var(--s1) 0;
  }
  .head {
    display: flex;
    justify-content: space-between;
    margin-bottom: 2px;
  }
  .value {
    color: var(--text-dim);
  }
  .changed .value {
    color: var(--text);
    font-weight: var(--w-medium);
  }
  .track {
    position: relative;
    height: 20px;
    cursor: pointer;
    touch-action: none;
    border-radius: var(--radius);
  }
  .rail,
  .fill {
    position: absolute;
    top: 9px;
    height: 2px;
    border-radius: 1px;
  }
  .rail {
    left: 0;
    right: 0;
    background: var(--line-strong);
    opacity: 0.5;
  }
  .fill {
    background: var(--accent);
  }
  .tick {
    position: absolute;
    top: 6px;
    width: 1px;
    height: 8px;
    background: var(--line-strong);
  }
  .thumb {
    position: absolute;
    top: 3px;
    width: 14px;
    height: 14px;
    margin-left: -7px;
    border-radius: 50%;
    background: var(--raised);
    border: 1px solid var(--line-strong);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.2);
    transition: transform var(--motion) ease-out;
  }
  .dragging .thumb {
    transform: scale(1.15);
  }
</style>
