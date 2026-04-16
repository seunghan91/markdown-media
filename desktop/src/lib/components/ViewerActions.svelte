<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { ViewerData } from '$lib/types';

  export let data: ViewerData | null = null;

  const dispatch = createEventDispatcher<{
    stats: void;
    diff: void;
    copied: { kind: 'markdown' | 'html' };
    exported: { format: 'json' | 'html' | 'txt' };
  }>();

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
    const title = data?.metadata?.title ?? 'document';
    return title.replace(/\.[^.]+$/, '');
  }

  function exportAs(format: 'json' | 'html' | 'txt') {
    if (!data) return;
    const base = baseFilename();
    switch (format) {
      case 'json': {
        const payload = JSON.stringify(
          { metadata: data.metadata, markdown: data.markdown, html: data.html },
          null,
          2
        );
        downloadBlob(payload, `${base}.json`, 'application/json');
        break;
      }
      case 'html': {
        const full = `<!doctype html><meta charset="utf-8"><title>${base}</title>${data.html}`;
        downloadBlob(full, `${base}.html`, 'text/html');
        break;
      }
      case 'txt': {
        // Strip markdown syntax for plain text output — rough but useful for
        // pasting into email/chat.
        const plain = data.markdown
          .replace(/<mark>([^<]*)<\/mark>/g, '$1')
          .replace(/<\/?u>/g, '')
          .replace(/~~([^~]+)~~/g, '$1')
          .replace(/\*{1,3}([^*]+)\*{1,3}/g, '$1')
          .replace(/^#{1,6}\s+/gm, '')
          .replace(/\[이미지:\s*([^\]]+)\]/g, '[$1]');
        downloadBlob(plain, `${base}.txt`, 'text/plain');
        break;
      }
    }
    dispatch('exported', { format });
    exportOpen = false;
  }
</script>

<div class="actions" aria-label="문서 액션">
  <button
    type="button"
    class="action-btn"
    disabled={!data}
    on:click={copyMarkdown}
    aria-label="마크다운 복사"
    title="마크다운을 클립보드로 복사"
  >
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
      <rect x="9" y="9" width="11" height="11" rx="2" stroke="currentColor" stroke-width="1.6" />
      <path
        d="M5 15H4a2 2 0 01-2-2V5a2 2 0 012-2h8a2 2 0 012 2v1"
        stroke="currentColor"
        stroke-width="1.6"
      />
    </svg>
    <span>{copyFeedback ?? '복사'}</span>
  </button>

  <button
    type="button"
    class="action-btn"
    disabled={!data}
    on:click={() => dispatch('stats')}
    aria-label="문서 통계"
    title="문단·표·이미지 수치를 확인"
  >
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
      <line x1="18" y1="20" x2="18" y2="10" stroke="currentColor" stroke-width="1.6" />
      <line x1="12" y1="20" x2="12" y2="4" stroke="currentColor" stroke-width="1.6" />
      <line x1="6" y1="20" x2="6" y2="14" stroke="currentColor" stroke-width="1.6" />
    </svg>
    <span>통계</span>
  </button>

  <button
    type="button"
    class="action-btn"
    disabled={!data}
    on:click={() => dispatch('diff')}
    aria-label="다른 문서와 비교"
    title="두 번째 HWP/문서를 골라 신구 대조"
  >
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
      <path d="M8 3v18" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <path d="M16 3v18" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <path d="M5 7l3-4 3 4" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" fill="none" />
      <path d="M13 17l3 4 3-4" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" fill="none" />
    </svg>
    <span>비교</span>
  </button>

  <div class="export-wrapper">
    <button
      type="button"
      class="action-btn"
      disabled={!data}
      aria-haspopup="menu"
      aria-expanded={exportOpen}
      on:click={() => (exportOpen = !exportOpen)}
      title="다른 포맷으로 내보내기"
    >
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
        <path d="M12 3v12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        <path
          d="M6 9l6 6 6-6"
          stroke="currentColor"
          stroke-width="1.6"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
        <path d="M4 21h16" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
      <span>내보내기</span>
    </button>
    {#if exportOpen}
      <div class="menu" role="menu">
        <button role="menuitem" on:click={() => exportAs('json')}>JSON</button>
        <button role="menuitem" on:click={() => exportAs('html')}>HTML</button>
        <button role="menuitem" on:click={() => exportAs('txt')}>TXT (plain)</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .actions {
    display: inline-flex;
    gap: var(--space-2);
    align-items: center;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: 999px;
    background: var(--color-bg-card);
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
    cursor: pointer;
    transition: background var(--duration-fast) var(--ease-default);
  }

  .action-btn:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
  }

  .action-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .action-btn svg {
    color: var(--color-label-secondary);
  }

  .export-wrapper {
    position: relative;
  }

  .menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    display: flex;
    flex-direction: column;
    min-width: 160px;
    padding: 4px;
    background: var(--color-bg-card);
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: var(--radius-sm);
    box-shadow: var(--card-shadow-hover);
    z-index: 10;
  }

  .menu button {
    padding: 8px 12px;
    text-align: left;
    border: 0;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
    cursor: pointer;
  }

  .menu button:hover {
    background: var(--color-fill-quaternary);
  }
</style>
