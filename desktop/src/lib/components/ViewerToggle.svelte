<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { ViewerMode } from '$lib/types';

  export let mode: ViewerMode = 'render';

  const dispatch = createEventDispatcher<{
    change: ViewerMode;
  }>();

  const items: { value: ViewerMode; label: string }[] = [
    { value: 'render', label: '렌더' },
    { value: 'split', label: '나란히' },
    { value: 'source', label: '소스' },
    { value: 'fidelity', label: '원본' }
  ];
</script>

<div class="toggle liquid-glass-segment" role="tablist" aria-label="뷰어 표시 모드">
  {#each items as item}
    <button
      type="button"
      class:active={item.value === mode}
      role="tab"
      aria-selected={item.value === mode}
      on:click={() => dispatch('change', item.value)}
    >
      {item.label}
    </button>
  {/each}
</div>

<style>
  .toggle {
    display: inline-flex;
    gap: 4px;
    padding: 4px;
  }

  button {
    border: 0;
    border-radius: 999px;
    padding: 8px 14px;
    color: var(--color-label-secondary);
    background: transparent;
  }

  button.active {
    color: white;
    background: var(--color-accent);
  }
</style>
