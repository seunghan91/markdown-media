<script lang="ts">
  import DropZone from '$lib/components/DropZone.svelte';
  import HwpEditPanel from '$lib/components/HwpEditPanel.svelte';
  import { pickFileWithDialog } from '$lib/utils/ipc';

  let sourcePath: string | null = null;
  let error = '';
  let panelOpen = false;

  async function pickAndOpen() {
    const picked = await pickFileWithDialog({
      title: 'HWP 파일 선택',
      filters: [{ name: 'HWP 문서', extensions: ['hwp', 'hwpx'] }],
    });
    if (!picked || Array.isArray(picked)) return;
    sourcePath = picked;
    panelOpen = true;
  }

  function handlePaths(paths: string[]) {
    const first = paths[0];
    if (!first) return;
    if (!/\.(hwp|hwpx)$/i.test(first)) {
      error = 'HWP / HWPX 파일만 편집할 수 있습니다.';
      return;
    }
    error = '';
    sourcePath = first;
    panelOpen = true;
  }
</script>

<div class="page">
  <header class="head">
    <div>
      <h2>HWP 편집</h2>
      <p class="sub">
        vendor된 <code>rhwp 0.7.2</code>로 HWP 문단을 직접 수정하고 새 파일로 저장합니다. 원본은 보존됩니다.
      </p>
    </div>
    {#if sourcePath}
      <button type="button" class="reopen" on:click={() => (panelOpen = true)}>
        편집 패널 다시 열기
      </button>
    {/if}
  </header>

  {#if error}
    <div class="err">{error}</div>
  {/if}

  {#if !sourcePath}
    <DropZone
      title="HWP 파일을 드롭하거나 클릭해서 선택"
      description=".hwp / .hwpx 파일을 열면 rhwp 편집 패널이 열립니다."
      on:paths={(event) => handlePaths(event.detail.paths)}
    />
    <div class="cta-row">
      <button type="button" class="primary" on:click={pickAndOpen}>HWP 파일 선택</button>
    </div>
  {:else}
    <div class="info">
      <div class="label">현재 파일</div>
      <code class="path">{sourcePath}</code>
      <button type="button" class="ghost" on:click={() => { sourcePath = null; }}>다른 파일</button>
    </div>
  {/if}

  <HwpEditPanel
    open={panelOpen}
    {sourcePath}
    on:close={() => (panelOpen = false)}
  />
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 900px;
    margin: 0 auto;
    padding: var(--space-4);
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
    border-radius: var(--radius-card);
    background: var(--color-bg-card);
    box-shadow: var(--card-shadow);
    border: 1px solid var(--color-separator-non-opaque);
  }

  h2 {
    margin: 0;
    font-size: var(--text-title2);
    color: var(--color-label-primary);
  }

  .sub {
    margin: 6px 0 0;
    font-size: var(--text-footnote-size);
    color: var(--color-label-secondary);
    max-width: 52ch;
    line-height: 1.5;
  }

  .sub code {
    padding: 1px 5px;
    border-radius: var(--radius-xs);
    background: var(--color-fill-quaternary);
    font-size: 0.92em;
  }

  .reopen {
    padding: 6px 14px;
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: 999px;
    background: var(--color-bg-card);
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
    cursor: pointer;
  }

  .err {
    padding: var(--space-2) var(--space-4);
    border-radius: var(--radius-xs);
    background: color-mix(in srgb, var(--color-error) 12%, transparent);
    color: var(--color-error);
    font-size: var(--text-footnote-size);
  }

  .cta-row {
    display: flex;
    justify-content: center;
  }

  .primary {
    padding: 10px 22px;
    border: 0;
    border-radius: 999px;
    background: var(--color-accent);
    color: white;
    font-size: var(--text-body-size);
    cursor: pointer;
  }

  .info {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-card);
    background: var(--color-bg-card);
    border: 1px solid var(--color-separator-non-opaque);
  }

  .label {
    font-size: var(--text-caption2-size);
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--color-label-tertiary);
    text-transform: uppercase;
  }

  .path {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--text-caption1-size);
    color: var(--color-label-primary);
    word-break: break-all;
  }

  .ghost {
    padding: 4px 12px;
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: 999px;
    background: transparent;
    color: var(--color-label-secondary);
    font-size: var(--text-caption1-size);
    cursor: pointer;
  }
</style>
