<script lang="ts">
  import { ui } from "../lib/app.svelte";
</script>

{#if ui.dropping}
  <div class="overlay" aria-hidden="true">
    <p>Drop to open</p>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: var(--s3);
    display: grid;
    place-items: center;
    border: 2px dashed var(--accent);
    border-radius: calc(var(--radius) * 2);
    background: color-mix(in srgb, var(--bg) 82%, transparent);
    z-index: 30;
    pointer-events: none;
    animation: fade var(--motion) ease-out;
  }
  p {
    padding: var(--s2) var(--s4);
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--on-accent);
    font-weight: var(--w-medium);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
</style>
