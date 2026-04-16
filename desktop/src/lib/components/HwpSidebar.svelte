<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { ViewerData } from '$lib/types';

  /**
   * Persistent left-column toolbar. Hosts every action the viewer
   * exposes: the four MDM extraction actions (copy / diff / notes /
   * export) plus one HWP-only action (편집) at the bottom, separated
   * by a divider. HWP 편집 is disabled unless the active file is
   * .hwp / .hwpx.
   */
  export let data: ViewerData | null = null;
  export let sourcePath: string | null = null;

  const dispatch = createEventDispatcher<{
    diff: void;
    notes: void;
    editHwp: void;
    copied: { kind: 'markdown' };
    exported: { format: 'json' | 'html' | 'txt' };
  }>();

  $: isHwp = !!sourcePath && /\.(hwp|hwpx)$/i.test(sourcePath);

  let copyFeedback: string | null = null;
  let exportOpen = false;

  async function copyMarkdown() {
    if (!data) return;
    try {
      await navigator.clipboard.writeText(data.markdown);
      dispatch('copied', { kind: 'markdown' });
      copyFeedback = '복사됨';
    } catch {
      copyFeedback = '복사 실패';
    }
    setTimeout(() => (copyFeedback = null), 1500);
  }

  function downloadBlob(content: string, filename: string, mime: string) {
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  }

  function baseFilename(): string {
    return (data?.metadata?.title ?? 'document').replace(/\.[^.]+$/, '');
  }

  function exportAs(format: 'json' | 'html' | 'txt') {
    if (!data) return;
    const base = baseFilename();
    if (format === 'json') {
      downloadBlob(
        JSON.stringify(
          { metadata: data.metadata, markdown: data.markdown, html: data.html },
          null,
          2
        ),
        `${base}.json`,
        'application/json'
      );
    } else if (format === 'html') {
      downloadBlob(
        `<!doctype html><meta charset="utf-8"><title>${base}</title>${data.html}`,
        `${base}.html`,
        'text/html'
      );
    } else {
      const plain = data.markdown
        .replace(/<mark>([^<]*)<\/mark>/g, '$1')
        .replace(/<\/?u>/g, '')
        .replace(/~~([^~]+)~~/g, '$1')
        .replace(/\*{1,3}([^*]+)\*{1,3}/g, '$1')
        .replace(/^#{1,6}\s+/gm, '')
        .replace(/\[이미지:\s*([^\]]+)\]/g, '[$1]');
      downloadBlob(plain, `${base}.txt`, 'text/plain');
    }
    dispatch('exported', { format });
    exportOpen = false;
  }
</script>

<aside class="viewer-sidebar" aria-label="뷰어 도구">
  <button
    type="button"
    class="sb-btn"
    disabled={!data}
    on:click={copyMarkdown}
    title="마크다운을 클립보드로 복사"
  >
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
      <rect x="9" y="9" width="11" height="11" rx="2" stroke="currentColor" stroke-width="1.6" />
      <path d="M5 15H4a2 2 0 01-2-2V5a2 2 0 012-2h8a2 2 0 012 2v1" stroke="currentColor" stroke-width="1.6" />
    </svg>
    <span>{copyFeedback ?? '복사'}</span>
  </button>

  <button
    type="button"
    class="sb-btn"
    disabled={!data}
    on:click={() => dispatch('diff')}
    title="두 번째 문서와 신구 대조"
  >
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
      <path d="M8 3v18" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <path d="M16 3v18" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <path d="M5 7l3-4 3 4" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" fill="none" />
      <path d="M13 17l3 4 3-4" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" fill="none" />
    </svg>
    <span>비교</span>
  </button>

  <button
    type="button"
    class="sb-btn"
    disabled={!data}
    on:click={() => dispatch('notes')}
    title="사이드카에 메모 저장 (원본 HWP 불변)"
  >
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
      <path
        d="M20 12l-6 6H5a1 1 0 01-1-1V5a1 1 0 011-1h14a1 1 0 011 1v7z"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linejoin="round"
      />
      <path d="M14 18v-6h6" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
    </svg>
    <span>메모</span>
  </button>

  <div class="export-wrapper">
    <button
      type="button"
      class="sb-btn"
      disabled={!data}
      aria-haspopup="menu"
      aria-expanded={exportOpen}
      on:click={() => (exportOpen = !exportOpen)}
      title="JSON / HTML / TXT로 내보내기"
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
        <path d="M12 3v12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        <path d="M6 9l6 6 6-6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
        <path d="M4 21h16" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
      <span>내보내기</span>
    </button>
    {#if exportOpen}
      <div class="menu" role="menu">
        <button role="menuitem" on:click={() => exportAs('json')}>JSON</button>
        <button role="menuitem" on:click={() => exportAs('html')}>HTML</button>
        <button role="menuitem" on:click={() => exportAs('txt')}>TXT</button>
      </div>
    {/if}
  </div>

  <div class="divider"></div>

  <button
    type="button"
    class="sb-btn primary"
    disabled={!isHwp}
    on:click={() => dispatch('editHwp')}
    title={isHwp
      ? 'rhwp로 HWP 문단을 직접 편집하고 새 파일로 저장'
      : 'HWP / HWPX 파일을 먼저 열어주세요'}
  >
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
      <path d="M12 20h9" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <path
        d="M16.5 3.5a2.121 2.121 0 113 3L7 19l-4 1 1-4L16.5 3.5z"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linejoin="round"
      />
    </svg>
    <span>HWP 편집</span>
  </button>
</aside>

<style>
  .viewer-sidebar {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    width: 180px;
    min-width: 180px;
    border-right: 1px solid var(--color-separator-non-opaque);
    background: var(--color-bg-card);
  }

  .divider {
    margin: var(--space-2) 0;
    height: 1px;
    background: var(--color-separator-non-opaque);
  }

  .export-wrapper {
    position: relative;
  }

  .menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    display: flex;
    flex-direction: column;
    padding: 4px;
    background: var(--color-bg-card);
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: var(--radius-sm);
    box-shadow: var(--card-shadow-hover);
    z-index: 10;
  }

  .menu button {
    padding: 6px 10px;
    border: 0;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
    text-align: left;
    cursor: pointer;
  }

  .menu button:hover {
    background: var(--color-fill-quaternary);
  }

  .sb-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: var(--radius-sm);
    background: var(--color-bg-card);
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
    text-align: left;
    cursor: pointer;
    transition: background var(--duration-fast) var(--ease-default);
  }

  .sb-btn svg {
    color: var(--color-label-secondary);
    flex-shrink: 0;
  }

  .sb-btn:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
  }

  .sb-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .sb-btn.primary {
    background: var(--color-accent);
    color: white;
    border-color: var(--color-accent);
    font-weight: 600;
  }

  .sb-btn.primary svg {
    color: white;
  }

  .sb-btn.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-accent) 88%, black);
  }

</style>
