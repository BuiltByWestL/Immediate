<script lang="ts">
  import Mic from "@lucide/svelte/icons/mic";
  import Pause from "@lucide/svelte/icons/pause";
  import Square from "@lucide/svelte/icons/square";
  import Trash from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import { closeRecorder, rec, startRecording, stopRecording, togglePause } from "../lib/record.svelte";
  import { fmtTime } from "../modes/media/session.svelte";

  let strip: HTMLCanvasElement;
  let width = $state(0);

  // Live level history, newest on the right.
  $effect(() => {
    const h = rec.history;
    if (!strip || !width) return;
    const dpr = window.devicePixelRatio || 1;
    const H = 72;
    strip.width = Math.round(width * dpr);
    strip.height = H * dpr;
    const c = strip.getContext("2d")!;
    c.setTransform(dpr, 0, 0, dpr, 0, 0);
    c.clearRect(0, 0, width, H);
    c.fillStyle = getComputedStyle(document.documentElement).getPropertyValue("--accent");
    const bar = 3;
    const n = Math.floor(width / bar);
    const data = h.slice(-n);
    data.forEach((v, i) => {
      const amp = Math.max(1, Math.sqrt(v) * (H - 4));
      c.fillRect(width - (data.length - i) * bar, (H - amp) / 2, bar - 1, amp);
    });
  });

  const db = $derived(rec.peak > 0 ? 20 * Math.log10(rec.peak) : -60);
  const meter = $derived(Math.max(0, Math.min(1, (db + 60) / 60)));
</script>

<div class="recorder">
  {#if rec.phase === "ready"}
    <button type="button" class="icon-btn close" aria-label="Close" title="Close (Esc)" onclick={closeRecorder}><X size={18} strokeWidth={1.5} /></button>
  {/if}

  <div class="center">
    <p class="time num">{fmtTime(rec.seconds, true)}</p>

    <div class="strip" bind:clientWidth={width}>
      <canvas bind:this={strip} style:width="{width}px" style:height="72px"></canvas>
    </div>
    <div class="meter" aria-hidden="true"><div class="level" class:hot={db > -3} style:width="{meter * 100}%"></div></div>

    {#if rec.phase === "ready"}
      <button type="button" class="record" onclick={startRecording} aria-label="Start recording" title="Start recording (R)">
        <Mic size={28} strokeWidth={1.75} />
      </button>
      {#if rec.devices.length > 1}
        <label class="device">
          <span class="dim">Microphone</span>
          <select class="field" bind:value={rec.device}>
            {#each rec.devices as d (d)}<option value={d}>{d}</option>{/each}
          </select>
        </label>
      {:else if rec.devices.length === 1}
        <p class="dim">{rec.devices[0]}</p>
      {:else}
        <p class="dim">No microphone found.</p>
      {/if}
      <p class="hint dim">Recordings save as you go, in Music › Immediate Recordings.</p>
    {:else}
      <div class="controls">
        <button type="button" class="btn lg" onclick={() => stopRecording(false)} disabled={rec.phase !== "recording"}>
          <Trash size={16} strokeWidth={1.5} />Discard
        </button>
        <button type="button" class="btn lg" onclick={togglePause} disabled={rec.phase !== "recording"}>
          {#if rec.paused}<Mic size={16} strokeWidth={1.5} />Resume{:else}<Pause size={16} strokeWidth={1.5} />Pause{/if}
        </button>
        <button type="button" class="btn lg primary" onclick={() => stopRecording(true)} disabled={rec.phase !== "recording"} title="Stop and edit (Space)">
          <Square size={14} strokeWidth={2} />Done
        </button>
      </div>
      <p class="hint dim">{rec.paused ? "Paused" : "Recording…"}</p>
    {/if}
  </div>
</div>

<style>
  .recorder {
    position: relative;
    flex: 1;
    display: grid;
    place-items: center;
    background: var(--canvas);
  }
  .close {
    position: absolute;
    top: var(--s3);
    right: var(--s3);
  }
  .center {
    width: min(560px, 90%);
    display: grid;
    justify-items: center;
    gap: var(--s4);
  }
  .time {
    font-size: 40px;
    font-weight: var(--w-medium);
    letter-spacing: -0.01em;
  }
  .strip {
    width: 100%;
  }
  canvas {
    display: block;
  }
  .meter {
    width: 100%;
    height: 4px;
    border-radius: 2px;
    background: var(--hover);
    overflow: hidden;
  }
  .level {
    height: 100%;
    background: var(--accent);
    transition: width 60ms linear;
  }
  .level.hot {
    background: var(--danger);
  }
  .record {
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    border-radius: 50%;
    border: 0;
    background: var(--danger);
    color: #fff;
    cursor: pointer;
    box-shadow: 0 0 0 4px var(--bg), 0 0 0 5px var(--line);
  }
  .record:hover {
    filter: brightness(1.08);
  }
  .device {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .device select {
    max-width: 320px;
  }
  .controls {
    display: flex;
    gap: var(--s2);
  }
  .hint {
    font-size: var(--text-sm);
  }
</style>
