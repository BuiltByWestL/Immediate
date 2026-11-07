<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  type UpdateInfo = { version: string; current_version: string; notes: string | null };

  let update = $state<UpdateInfo | null>(null);
  let phase = $state<"idle" | "installing" | "error">("idle");
  let error = $state("");

  onMount(() => {
    // Check quietly a moment after startup so it never competes with opening a file.
    const t = setTimeout(async () => {
      try {
        update = await invoke<UpdateInfo | null>("update_check");
      } catch {
        // Offline, rate-limited, or no access: say nothing. Settings → Check for updates reports errors.
      }
    }, 3000);
    return () => clearTimeout(t);
  });

  async function install() {
    phase = "installing";
    try {
      await invoke("update_install"); // the app restarts into the new version
    } catch (e) {
      phase = "error";
      error = String(e);
    }
  }
</script>

{#if update}
  <div class="bar" role="status">
    {#if phase === "error"}
      <span>Couldn’t install the update: {error}</span>
      <button type="button" onclick={install}>Try again</button>
    {:else if phase === "installing"}
      <span>Installing Immediate {update.version}… it will reopen by itself.</span>
    {:else}
      <span>Immediate <span class="num">{update.version}</span> is available.</span>
      <button type="button" class="primary" onclick={install}>Install & restart</button>
    {/if}
    {#if phase !== "installing"}
      <button type="button" onclick={() => (update = null)}>Later</button>
    {/if}
  </div>
{/if}

<style>
  .bar {
    position: fixed;
    left: 50%;
    bottom: var(--s4);
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s2) var(--s2) var(--s2) var(--s4);
    background: var(--panel-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    white-space: nowrap;
  }
  button {
    height: 28px;
    padding: 0 var(--s3);
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text);
    font-weight: var(--w-medium);
    cursor: pointer;
  }
  button:hover {
    border-color: var(--text-dim);
  }
  button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
</style>
