<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { AppMode } from '$lib/types';

  export let collapsed = false;
  export let currentMode: AppMode = 'convert';

  const dispatch = createEventDispatcher<{
    navigate: AppMode;
    settings: void;
  }>();

  const primaryItems: { mode: AppMode; label: string; svg: string }[] = [
    { mode: 'convert', label: '변환', svg: '<path d="M7 16V4m0 0L3 8m4-4l4 4m6-4v12m0 0l4-4m-4 4l-4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>' },
    { mode: 'viewer', label: '뷰어', svg: '<path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7S2 12 2 12z" stroke="currentColor" stroke-width="1.5" fill="none"/><circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.5" fill="none"/>' },
    { mode: 'batch', label: '대량', svg: '<path d="M9 5H7a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 012-2h2a2 2 0 012 2M9 5h6m-3 5v6m-3-3h6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>' },
    { mode: 'export', label: 'MD변환', svg: '<path d="M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2M7 10l5 5 5-5M12 15V3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>' },
  ];
</script>

<aside class:collapsed class="sidebar liquid-glass-sidebar panel" aria-label="주요 모드 탐색">
  <nav class="rail" aria-label="앱 모드">
    {#each primaryItems as item}
      <button
        type="button"
        class:selected={item.mode === currentMode}
        class="icon-button"
        title={item.label}
        aria-current={item.mode === currentMode ? 'page' : undefined}
        on:click={() => dispatch('navigate', item.mode)}
      >
        <span class="icon-wrap">
          <svg width="20" height="20" viewBox="0 0 24 24">{@html item.svg}</svg>
        </span>
        <span class="icon-label">{item.label}</span>
      </button>
    {/each}

    <div class="spacer"></div>

    <button
      type="button"
      class="icon-button utility"
      title="설정"
      aria-label="설정"
      on:click={() => dispatch('settings')}
    >
      <span class="icon-wrap">
        <svg width="20" height="20" viewBox="0 0 24 24">
          <path d="M12.22 2h-.44a2 2 0 00-2 2v.18a2 2 0 01-1 1.73l-.43.25a2 2 0 01-2 0l-.15-.08a2 2 0 00-2.73.73l-.22.38a2 2 0 00.73 2.73l.15.1a2 2 0 011 1.72v.51a2 2 0 01-1 1.74l-.15.09a2 2 0 00-.73 2.73l.22.38a2 2 0 002.73.73l.15-.08a2 2 0 012 0l.43.25a2 2 0 011 1.73V20a2 2 0 002 2h.44a2 2 0 002-2v-.18a2 2 0 011-1.73l.43-.25a2 2 0 012 0l.15.08a2 2 0 002.73-.73l.22-.39a2 2 0 00-.73-2.73l-.15-.08a2 2 0 01-1-1.74v-.5a2 2 0 011-1.74l.15-.09a2 2 0 00.73-2.73l-.22-.38a2 2 0 00-2.73-.73l-.15.08a2 2 0 01-2 0l-.43-.25a2 2 0 01-1-1.73V4a2 2 0 00-2-2z" stroke="currentColor" stroke-width="1.5" fill="none"/>
          <circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.5" fill="none"/>
        </svg>
      </span>
      <span class="icon-label">설정</span>
    </button>
  </nav>
</aside>

<style>
  .sidebar {
    display: flex;
    height: 100%;
    width: 68px;
    padding: 12px 6px;
    border-left: 0;
    border-top: 0;
    border-bottom: 0;
    border-radius: 0;
    transition: width var(--duration-normal) var(--ease-default);
  }

  .sidebar.collapsed {
    width: 52px;
  }

  .rail {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
  }

  .spacer {
    flex: 1;
  }

  .icon-button {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    width: 100%;
    min-height: 48px;
    padding: 6px 0;
    border: 0;
    border-radius: var(--radius-sm);
    color: var(--color-label-tertiary);
    background: transparent;
    cursor: pointer;
    transition:
      background var(--duration-fast) var(--ease-default),
      color var(--duration-fast) var(--ease-default);
  }

  .icon-label {
    font-size: 10px;
    line-height: 1;
    font-weight: 500;
    letter-spacing: 0.02em;
  }

  .icon-button:hover {
    color: var(--color-label-primary);
    background: var(--color-fill-quaternary);
  }

  .icon-button.selected {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
  }

  .icon-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-sm);
  }

  .selected .icon-wrap {
    background: var(--color-accent);
    color: var(--color-bg-primary);
    border-radius: var(--radius-sm);
  }

  @media (max-width: 960px) {
    .sidebar {
      height: auto;
      width: 100%;
      border-radius: 0;
      padding: 6px;
    }

    .rail {
      flex-direction: row;
      align-items: center;
      overflow-x: auto;
    }

    .spacer {
      display: none;
    }
  }
</style>
