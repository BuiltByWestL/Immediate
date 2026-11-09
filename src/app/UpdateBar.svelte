<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";
  import { inTauri, ui, updateSettings } from "../lib/app.svelte";
  import { checkForUpdates, installUpdate, updates } from "../lib/updates.svelte";

  const DAY = 24 * 60 * 60 * 1000;

  onMount(() => {
    if (!inTauri || !ui.settings.checkUpdates) return;
    // At most once a day once Immediate is known to be up to date; while an update is waiting
    // (or the last check failed), every launch looks again so the offer stays in view.
    if (Date.now() - ui.settings.lastUpdateCheck < DAY) return;
    // Check quietly a moment after startup so it never competes with opening a file.
    const t = setTimeout(async () => {
      await checkForUpdates(false);
      if (updates.lastResult === "current") updateSettings({ lastUpdateCheck: Date.now() });
    }, 3000);
    return () => clearTimeout(t);
  });
</script>

{#if updates.available && !updates.hidden}
  <div class="bar">
    {#if updates.phase === "installing"}
      <span>Installing Immediate <span class="num">{updates.available.version}</span>… it will reopen by itself.</span>
    {:else if updates.phase === "error"}
      <span class="error">Couldn’t install the update: {updates.error}</span>
      <button type="button" class="btn" onclick={installUpdate}>Try again</button>
      <button type="button" class="btn" onclick={() => (updates.hidden = true)}>Later</button>
    {:else}
      <span>Immediate <span class="num">{updates.available.version}</span> is available.</span>
      <button type="button" class="btn" onclick={() => openUrl(`https://github.com/BuiltByWestL/Immediate/releases/tag/v${updates.available?.version}`).catch(() => {})}>What’s new</button>
      <button type="button" class="btn primary" onclick={installUpdate}>Install & restart</button>
      <button type="button" class="btn" onclick={() => (updates.hidden = true)}>Later</button>
    {/if}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: var(--s2);
    padding: var(--s1) var(--s1) var(--s1) var(--s4);
    min-height: 36px;
    background: var(--raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.12);
    white-space: nowrap;
  }
  .bar span {
    margin-right: var(--s2);
  }
  .error {
    color: var(--danger);
  }
</style>
