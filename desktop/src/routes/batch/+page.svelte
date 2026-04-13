<script lang="ts">
  import DropZone from '$lib/components/DropZone.svelte';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import { openInViewer } from '$lib/stores/viewer';
  import { batchConvert, convertFile } from '$lib/utils/ipc';
  import type { BatchItemResult, BatchResult, ExportFormat } from '$lib/types';

  let files: File[] = [];
  let progress = 0;
  let loading = false;
  let outputFormat: ExportFormat = 'md';
  let outputDir = './exports';
  let result: BatchResult | null = null;

  async function runBatch() {
    if (files.length === 0) return;
    loading = true;
    progress = 12;

    try {
      const paths = files.map((file) => (file as File & { path?: string }).path ?? file.name);
      progress = 56;
      result = await batchConvert(paths, outputFormat, outputDir);
      progress = 100;
    } finally {
      loading = false;
    }
  }

  /** 성공한 결과 행 클릭 → 뷰어에서 열기 */
  async function openResultInViewer(item: BatchItemResult) {
    if (item.status !== 'success') return;
    const target = item.outputPath ?? item.inputPath;
    try {
      const data = await convertFile(target, 'markdown');
      await openInViewer(data, target);
    } catch {
      // 변환 실패 시 무시
    }
  }
</script>

<div class="batch-page">
  <DropZone
    directory={true}
    multiple={true}
    title="폴더 또는 여러 파일을 드롭하세요"
    description="배치 모드는 전체 선택, 개별 선택, 결과 집계를 포함합니다."
    buttonLabel="폴더 선택"
    on:files={(event) => (files = event.detail.files)}
  />

  <div class="control-grid">
    <div class="card control-card">
      <h3 class="card-title">설정</h3>
      <label class="form-field">
        <span class="field-label">출력 포맷</span>
        <select class="field-select" bind:value={outputFormat}>
          <option value="md">Markdown</option>
          <option value="docx">DOCX</option>
          <option value="hwpx">HWPX</option>
          <option value="pdf">PDF</option>
        </select>
      </label>

      <label class="form-field">
        <span class="field-label">출력 폴더</span>
        <input class="field-input" bind:value={outputDir} />
      </label>

      <button class="action-btn" disabled={files.length === 0 || loading} on:click={runBatch}>
        {loading ? '처리 중...' : '배치 시작'}
      </button>
    </div>

    <div class="card result-card">
      <h3 class="card-title">진행 상태</h3>
      <ProgressBar progress={progress} label="전체 진행률" />
      <div class="stat-grid">
        <div class="stat-item">
          <span class="stat-value">{files.length}</span>
          <span class="stat-label">대상 파일</span>
        </div>
        <div class="stat-item success">
          <span class="stat-value">{result?.success ?? 0}</span>
          <span class="stat-label">성공</span>
        </div>
        <div class="stat-item error">
          <span class="stat-value">{result?.failed ?? 0}</span>
          <span class="stat-label">실패</span>
        </div>
      </div>
    </div>
  </div>

  <div class="card table-card">
    <div class="table-header">
      <h3 class="card-title">배치 결과</h3>
      <span class="format-badge">{outputFormat.toUpperCase()}</span>
    </div>

    {#if result?.results?.length}
      <div class="table-body">
        {#each result.results as item}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class="table-row"
            class:clickable={item.status === 'success'}
            on:click={() => openResultInViewer(item)}
            on:keydown={(e) => e.key === 'Enter' && openResultInViewer(item)}
            role={item.status === 'success' ? 'button' : undefined}
            tabindex={item.status === 'success' ? 0 : undefined}
          >
            <span class="row-file">{item.inputPath.split('/').pop()}</span>
            <span class="row-status" class:success={item.status === 'success'} class:failed={item.status !== 'success'}>
              {item.status === 'success' ? '뷰어에서 보기' : '실패'}
            </span>
          </div>
        {/each}
      </div>
    {:else}
      <p class="empty-text">결과가 아직 없습니다.</p>
    {/if}
  </div>
</div>

<style>
  .batch-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 100%;
  }

  .card {
    padding: var(--space-5);
    border-radius: var(--radius-card);
    border: 1px solid var(--color-separator-non-opaque);
    background: var(--color-bg-card);
    box-shadow: var(--card-shadow);
  }

  .card-title {
    margin: 0 0 var(--space-3);
    font-size: var(--text-subheadline-size);
    font-weight: 600;
    color: var(--color-label-primary);
  }

  .control-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: var(--space-4);
  }

  .control-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .form-field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .field-label {
    font-size: var(--text-caption1-size);
    font-weight: 500;
    color: var(--color-label-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .field-select,
  .field-input {
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: var(--radius-sm);
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
    font-family: inherit;
    outline: none;
    transition: border-color var(--duration-fast) var(--ease-default);
  }

  .field-select:focus,
  .field-input:focus {
    border-color: var(--color-accent);
  }

  .action-btn {
    padding: var(--space-2) var(--space-4);
    border: 0;
    border-radius: var(--radius-sm);
    background: var(--color-accent);
    color: var(--color-bg-primary);
    font-size: var(--text-footnote-size);
    font-weight: 600;
    cursor: pointer;
    min-height: auto;
    min-width: auto;
    transition: opacity var(--duration-fast) var(--ease-default);
  }

  .action-btn:hover:not(:disabled) {
    opacity: 0.85;
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .result-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .stat-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-2);
  }

  .stat-item {
    text-align: center;
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    background: var(--color-fill-quaternary);
  }

  .stat-value {
    display: block;
    font-size: var(--text-title3);
    font-weight: 700;
    color: var(--color-label-primary);
  }

  .stat-item.success .stat-value { color: var(--color-success); }
  .stat-item.error .stat-value { color: var(--color-error); }

  .stat-label {
    font-size: var(--text-caption1-size);
    color: var(--color-label-tertiary);
  }

  .table-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .table-header .card-title {
    margin: 0;
  }

  .format-badge {
    font-size: var(--text-caption1-size);
    font-weight: 600;
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-xs);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
    color: var(--color-accent);
  }

  .table-body {
    display: flex;
    flex-direction: column;
  }

  .table-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--space-3) 0;
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  .table-row:last-child {
    border-bottom: 0;
  }

  .row-file {
    font-size: var(--text-footnote-size);
    color: var(--color-label-primary);
  }

  .row-status {
    font-size: var(--text-caption1-size);
    font-weight: 600;
    padding: 2px var(--space-2);
    border-radius: var(--radius-xs);
  }

  .row-status.success {
    color: var(--color-success);
    background: color-mix(in srgb, var(--color-success) 10%, transparent);
  }

  .row-status.failed {
    color: var(--color-error);
    background: color-mix(in srgb, var(--color-error) 10%, transparent);
  }

  .table-row.clickable {
    cursor: pointer;
    border-radius: var(--radius-xs);
    transition: background var(--duration-fast) var(--ease-default);
  }

  .table-row.clickable:hover {
    background: var(--color-fill-quaternary);
  }

  .empty-text {
    margin: 0;
    font-size: var(--text-footnote-size);
    color: var(--color-label-tertiary);
  }

  /* auto-fit handles all breakpoints */
</style>
