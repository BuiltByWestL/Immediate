<script lang="ts">
  import { SHORTCUTS } from "../lib/shortcuts";
</script>

<div class="groups">
  {#each SHORTCUTS as g (g.group)}
    <section>
      <h3>{g.group}</h3>
      <dl>
        {#each g.items as [keys, label] (label)}
          <dt>{label}</dt>
          <dd>
            {#each keys as k, i (i)}<kbd>{k}</kbd>{/each}
          </dd>
        {/each}
      </dl>
    </section>
  {/each}
</div>

<style>
  .groups {
    display: grid;
    gap: var(--s5);
  }
  h3 {
    margin-bottom: var(--s2);
    font-size: var(--text-sm);
    font-weight: var(--w-medium);
    color: var(--text-dim);
  }
  dl {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--s2) var(--s4);
    margin: 0;
    align-items: center;
  }
  dd {
    margin: 0;
    display: flex;
    gap: var(--s1);
    justify-content: flex-end;
  }
</style>
