<script lang="ts">
  import DropZone from '$lib/components/DropZone.svelte';
  import { exportMarkdown, markdownToHtml } from '$lib/utils/ipc';

  let markdown = '# 보고서 제목\n\n여기에 Markdown 내용을 입력하거나 파일을 드롭하세요.';
  let format: 'docx' | 'hwpx' | 'pdf' = 'docx';
  let template = '기본';
  let output = './exports/output.docx';
  let previewHtml = '';
  let status = '';

  $: void refreshPreview();
  $: output = `./exports/output.${format}`;

  async function refreshPreview() {
    previewHtml = await markdownToHtml(markdown);
  }

  async function handleFiles(files: File[]) {
    const first = files[0];
    if (!first) return;
    markdown = await first.text();
  }

  async function handleExport() {
    status = '내보내기 중...';
    try {
      await exportMarkdown(markdown, format, template, output);
      status = `완료: ${output}`;
    } catch (caught) {
      status = caught instanceof Error ? caught.message : '내보내기에 실패했습니다.';
    }
  }
</script>

<div class="export-page">
  <DropZone
    title="Markdown 파일을 드롭하거나 아래에 직접 편집하세요"
    description="MD 파일을 불러와 DOCX, HWPX, PDF로 저장합니다."
    on:files={(event) => handleFiles(event.detail.files)}
  />

  <div class="export-grid">
    <div class="card form-card">
      <h3 class="card-title">내보내기 설정</h3>

      <label class="form-field">
        <span class="field-label">포맷</span>
        <select class="field-select" bind:value={format}>
          <option value="docx">DOCX</option>
          <option value="hwpx">HWPX</option>
          <option value="pdf">PDF</option>
        </select>
      </label>

      <label class="form-field">
        <span class="field-label">템플릿</span>
        <select class="field-select" bind:value={template}>
          <option value="기본">기본</option>
          <option value="공문서">공문서</option>
          <option value="보고서">보고서</option>
        </select>
      </label>

      <label class="form-field">
        <span class="field-label">저장 경로</span>
        <input class="field-input" bind:value={output} />
      </label>

      <label class="form-field">
        <span class="field-label">Markdown 소스</span>
        <textarea class="field-textarea" bind:value={markdown} spellcheck="false"></textarea>
      </label>

      <button class="action-btn" on:click={handleExport}>변환 후 저장</button>
      {#if status}
        <div class="status-pill" class:success={status.startsWith('완료')} class:error={!status.startsWith('완료') && !status.includes('중')}>
          {status}
        </div>
      {/if}
    </div>

    <div class="card preview-card">
      <h3 class="card-title">미리보기</h3>
      <div class="preview-body">
        {@html previewHtml}
      </div>
    </div>
  </div>
</div>

<style>
  .export-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 1400px;
    margin: 0 auto;
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

  .export-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-4);
    align-items: start;
  }

  .form-card {
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

  .field-textarea {
    width: 100%;
    min-height: 200px;
    padding: var(--space-3);
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: var(--radius-sm);
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
    font-family: var(--font-mono);
    font-size: var(--text-footnote-size);
    line-height: 1.6;
    resize: vertical;
    outline: none;
    transition: border-color var(--duration-fast) var(--ease-default);
  }

  .field-textarea:focus {
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

  .action-btn:hover {
    opacity: 0.85;
  }

  .status-pill {
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-full);
    font-size: var(--text-caption1-size);
    font-weight: 500;
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  }

  .status-pill.success {
    color: var(--color-success);
    background: color-mix(in srgb, var(--color-success) 10%, transparent);
  }

  .status-pill.error {
    color: var(--color-error);
    background: color-mix(in srgb, var(--color-error) 10%, transparent);
  }

  .preview-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .preview-body {
    min-height: 400px;
    padding: var(--space-4);
    border-radius: var(--radius-sm);
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
    line-height: 1.7;
    overflow: auto;
  }

  @media (max-width: 960px) {
    .export-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
