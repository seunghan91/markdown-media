<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import DropZone from '$lib/components/DropZone.svelte';
  import FileList from '$lib/components/FileList.svelte';
  import QuickActions from '$lib/components/QuickActions.svelte';
  import { refreshHistory, historyEntries } from '$lib/stores/history';
  import { openInViewer } from '$lib/stores/viewer';
  import { convertFile, openFile } from '$lib/utils/ipc';
  import type { ConvertResult, HistoryEntry } from '$lib/types';

  let loading = false;
  let error = '';
  let result: ConvertResult | null = null;
  let selectedId: number | null = null;

  onMount(() => {
    refreshHistory();
  });

  async function convertPath(path: string) {
    loading = true;
    error = '';

    try {
      result = await convertFile(path, 'markdown');
      await refreshHistory();
      // 변환 성공 → 뷰어에서 결과 보기
      await openInViewer(result, path);
    } catch (caught) {
      if (typeof caught === 'string') {
        error = caught;
      } else if (caught instanceof Error) {
        error = caught.message;
      } else {
        error = JSON.stringify(caught);
      }
      console.error('[MDM convert error]', caught);
    } finally {
      loading = false;
    }
  }

  /** DropZone paths 이벤트 (Tauri dialog에서 실제 경로) */
  function handlePaths(paths: string[]) {
    const first = paths[0];
    if (first) convertPath(first);
  }

  /** DropZone files 이벤트 (브라우저 HTML input fallback) */
  function handleFiles(files: File[]) {
    const first = files[0];
    if (!first) return;
    const path = (first as File & { path?: string }).path ?? first.name;
    convertPath(path);
  }

  /** 히스토리 삭제 */
  function handleHistoryDelete(entry: HistoryEntry) {
    historyEntries.update((list) => list.filter((e) => e.id !== entry.id));
  }

  /** 히스토리 클릭 → 성공이면 뷰어에서 열기, 실패면 재변환 */
  async function handleHistorySelect(entry: HistoryEntry) {
    selectedId = entry.id;
    if (entry.status === 'success') {
      loading = true;
      try {
        const data = await convertFile(entry.filePath, 'markdown');
        await openInViewer(data, entry.filePath);
      } catch {
        // fallback: 뷰어로만 이동
        goto('/viewer');
      } finally {
        loading = false;
      }
    } else {
      convertPath(entry.filePath);
    }
  }
</script>

<div class="convert-page">
  <DropZone
    title="문서를 드래그하거나 클릭하세요"
    description="HWP, HWPX, PDF, DOCX 파일을 Markdown으로 변환합니다"
    on:paths={(event) => handlePaths(event.detail.paths)}
    on:files={(event) => handleFiles(event.detail.files)}
  />

  <QuickActions on:action={(event) => goto(`/${event.detail}`)} />

  <div class="bottom-grid">
    <!-- Result summary -->
    <div class="result-card">
      <h3 class="card-title">변환 결과</h3>

      {#if loading}
        <div class="status-pill loading">변환 중...</div>
      {:else if error}
        <div class="status-pill error">{error}</div>
      {:else if result}
        <div class="meta-grid">
          <div class="meta-item">
            <span class="meta-label">제목</span>
            <span class="meta-value">{result.metadata.title ?? '알 수 없음'}</span>
          </div>
          <div class="meta-item">
            <span class="meta-label">포맷</span>
            <span class="meta-value">{result.metadata.format}</span>
          </div>
          <div class="meta-item">
            <span class="meta-label">페이지</span>
            <span class="meta-value">{result.metadata.pageCount ?? '-'}</span>
          </div>
          <div class="meta-item">
            <span class="meta-label">이미지</span>
            <span class="meta-value">{result.images.length}</span>
          </div>
        </div>
        <pre class="preview-code">{result.markdown.slice(0, 500)}{result.markdown.length > 500 ? '...' : ''}</pre>
      {:else}
        <p class="empty-text">변환 결과가 여기에 표시됩니다.</p>
      {/if}
    </div>

    <!-- History -->
    <FileList
      entries={$historyEntries}
      {selectedId}
      on:select={(event) => handleHistorySelect(event.detail)}
      on:delete={(event) => handleHistoryDelete(event.detail)}
    />
  </div>
</div>

<style>
  .convert-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    max-width: 100%;
  }

  .bottom-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: var(--space-4);
  }

  .result-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-5);
    border-radius: var(--radius-card);
    border: 1px solid var(--color-separator-non-opaque);
    background: var(--color-bg-card);
    box-shadow: var(--card-shadow);
  }

  .card-title {
    margin: 0;
    font-size: var(--text-subheadline-size);
    font-weight: 600;
    color: var(--color-label-primary);
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-full);
    font-size: var(--text-caption1-size);
    font-weight: 500;
  }

  .status-pill.loading {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  }

  .status-pill.error {
    color: var(--color-error);
    background: color-mix(in srgb, var(--color-error) 10%, transparent);
  }

  .meta-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-2);
  }

  .meta-item {
    display: flex;
    justify-content: space-between;
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  .meta-label {
    font-size: var(--text-footnote-size);
    color: var(--color-label-tertiary);
  }

  .meta-value {
    font-size: var(--text-footnote-size);
    font-weight: 500;
    color: var(--color-label-primary);
  }

  .preview-code {
    margin: 0;
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    background: var(--color-fill-quaternary);
    font-family: var(--font-mono);
    font-size: var(--text-caption1-size);
    color: var(--color-label-secondary);
    line-height: 1.5;
    overflow: auto;
    max-height: 200px;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .empty-text {
    margin: 0;
    font-size: var(--text-footnote-size);
    color: var(--color-label-tertiary);
  }

  /* auto-fit handles all breakpoints */
</style>
