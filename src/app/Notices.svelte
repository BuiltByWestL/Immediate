<script lang="ts">
  // Bottom-center stack: the update offer and short-lived toasts share one place on screen.
  import X from "@lucide/svelte/icons/x";
  import { dismiss, ui } from "../lib/app.svelte";
  import UpdateBar from "./UpdateBar.svelte";
  import { tools } from "../lib/tools.svelte";

  const pct = $derived(tools.total ? Math.round((tools.received / tools.total) * 100) : 0);
  const MB = (n: number) => Math.round(n / 1e6);
</script>

<div class="stack" role="status" aria-live="polite">
  <UpdateBar />
  {#if tools.installing}
    <div class="toast pill">
      <span>
        {#if tools.phase === "verify"}Checking the download…
        {:else if tools.phase === "unpack"}Finishing setup…
        {:else}Setting up video & audio tools{#if tools.total} · <span class="num">{pct}% of {MB(tools.total)} MB</span>{/if}…{/if}
      </span>
    </div>
  {/if}
  {#each ui.toasts as t (t.id)}
    <div class="toast" class:error={t.tone === "error"}>
      <span>{t.text}</span>
      {#if t.action}
        <button type="button" class="btn" onclick={() => (t.action!.run(), dismiss(t.id))}>{t.action.label}</button>
      {/if}
      <button type="button" class="icon-btn" aria-label="Dismiss" onclick={() => dismiss(t.id)}><X size={14} strokeWidth={1.5} /></button>
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    left: 50%;
    bottom: var(--s4);
    transform: translateX(-50%);
    display: grid;
    justify-items: center;
    gap: var(--s2);
    z-index: 20;
    pointer-events: none;
    width: max-content;
    max-width: calc(100vw - 32px);
  }
  .stack > :global(*) {
    pointer-events: auto;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: var(--s2);
    min-height: 36px;
    padding: var(--s1) var(--s1) var(--s1) var(--s4);
    background: var(--raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.12);
    animation: fade var(--motion) ease-out;
  }
  .pill {
    padding-right: var(--s4);
  }
  .toast.error span {
    color: var(--danger);
  }
  .toast .icon-btn {
    width: 24px;
    height: 24px;
    color: var(--text-dim);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
</style>
