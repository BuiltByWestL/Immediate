<script lang="ts">
  import UpdateBar from "./app/UpdateBar.svelte";

  let dragging = $state(false);
</script>

<UpdateBar />

<main
  class="empty"
  class:dragging
  ondragenter={() => (dragging = true)}
  ondragleave={() => (dragging = false)}
  ondrop={() => (dragging = false)}
>
  <div class="prompt">
    <p class="title">Drop a file here, or click to open</p>
    <p class="hint">Photos, videos and audio</p>
    <div class="actions">
      <button type="button" class="primary">Open file</button>
      <button type="button">Paste</button>
      <button type="button">Record audio</button>
    </div>
  </div>
</main>

<style>
  .empty {
    height: 100vh;
    display: grid;
    place-items: center;
    background: var(--canvas);
    transition: background-color var(--motion) ease-out;
  }
  .empty.dragging {
    background: var(--panel);
  }
  .prompt {
    text-align: center;
  }
  .title {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: var(--w-medium);
  }
  .hint {
    margin: var(--s1) 0 var(--s6);
    color: var(--text-dim);
  }
  .actions {
    display: flex;
    gap: var(--s2);
    justify-content: center;
  }
  button {
    font-weight: var(--w-medium);
    height: 32px;
    padding: 0 var(--s4);
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--panel-raised);
    color: var(--text);
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
