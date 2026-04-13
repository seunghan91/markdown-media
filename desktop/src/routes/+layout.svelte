<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Toolbar from '$lib/components/Toolbar.svelte';
  import { setMode, sidebarCollapsed, toggleSidebar } from '$lib/stores/app';
  import { cycleViewerMode } from '$lib/stores/viewer';
  import type { AppMode } from '$lib/types';
  import '$lib/styles/tokens.css';
  import '$lib/styles/liquid-glass.css';
  import '$lib/styles/global.css';

  const pathToMode = (pathname: string): AppMode => {
    if (pathname.startsWith('/viewer')) return 'viewer';
    if (pathname.startsWith('/batch')) return 'batch';
    if (pathname.startsWith('/export')) return 'export';
    return 'convert';
  };

  $: currentMode = pathToMode($page.url.pathname);
  $: setMode(currentMode);

  async function navigate(mode: AppMode) {
    await goto(`/${mode}`);
  }

  function handleShortcut(event: KeyboardEvent) {
    const modifier = event.metaKey || event.ctrlKey;
    if (!modifier) return;

    if (event.key === '1') navigate('convert');
    if (event.key === '2') navigate('viewer');
    if (event.key === '3') navigate('batch');
    if (event.key === '4') navigate('export');
    if (event.key === '\\') toggleSidebar();
    if (event.key.toLowerCase() === 'd') cycleViewerMode();
  }

  onMount(() => {
    window.addEventListener('keydown', handleShortcut);
    return () => window.removeEventListener('keydown', handleShortcut);
  });
</script>

<div class="layout-root">
  <Sidebar
    collapsed={$sidebarCollapsed}
    {currentMode}
    on:navigate={(event) => navigate(event.detail)}
    on:settings={() => { /* TODO: open settings panel */ }}
  />

  <div class="layout-content">
    <Toolbar currentMode={currentMode} on:navigate={(event) => navigate(event.detail)} />
    <main class="layout-main">
      <slot />
    </main>
  </div>
</div>

<style>
  .layout-root {
    display: grid;
    grid-template-columns: auto 1fr;
    height: 100vh;
    overflow: hidden;
  }

  .layout-content {
    display: grid;
    grid-template-rows: auto 1fr;
    min-width: 0;
    overflow: hidden;
  }

  .layout-main {
    padding: 20px;
    overflow-y: auto;
    overflow-x: hidden;
    background: var(--color-bg-page);
  }
</style>
