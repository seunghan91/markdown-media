<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { AppMode } from '$lib/types';

  export let currentMode: AppMode = 'convert';

  const dispatch = createEventDispatcher<{
    navigate: AppMode;
    settings: void;
  }>();

  const items: { mode: AppMode; label: string }[] = [
    { mode: 'convert', label: '변환' },
    { mode: 'viewer', label: '뷰어' },
    { mode: 'batch', label: '대량' },
    { mode: 'export', label: 'MD변환' }
  ];
</script>

<header class="toolbar liquid-glass-toolbar panel" data-tauri-drag-region>
  <div class="segment liquid-glass-segment" role="tablist" aria-label="앱 모드 전환">
    {#each items as item}
      <button
        type="button"
        class:active={item.mode === currentMode}
        class="segment-button"
        role="tab"
        aria-selected={item.mode === currentMode}
        on:click={() => dispatch('navigate', item.mode)}
      >
        {item.label}
      </button>
    {/each}
  </div>
</header>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
    margin: 14px 14px 0;
    padding: 12px 16px;
    min-height: 52px;
    position: relative;
  }

  .segment {
    display: inline-flex;
    gap: 4px;
    padding: 4px;
  }

  .segment-button {
    border: 0;
    background: transparent;
    min-width: 80px;
    border-radius: 999px;
    padding: 8px 16px;
    color: var(--color-label-secondary);
    font-size: 14px;
    transition:
      background var(--duration-fast) var(--ease-default),
      color var(--duration-fast) var(--ease-default);
  }

  .segment-button.active {
    color: var(--color-bg-primary);
    background: var(--color-accent);
  }


  @media (max-width: 960px) {
    .toolbar {
      padding: 10px 12px;
    }

    .segment {
      overflow-x: auto;
    }
  }
</style>
