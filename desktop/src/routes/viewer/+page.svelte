<script lang="ts">
  import DropZone from '$lib/components/DropZone.svelte';
  import ViewerToggle from '$lib/components/ViewerToggle.svelte';
  import { setViewerMode, viewerData, viewerMode } from '$lib/stores/viewer';
  import { openFile, markdownToHtml } from '$lib/utils/ipc';
  import type { ViewerData } from '$lib/types';

  let loading = false;
  let error = '';
  let activeFileName = '선택된 파일 없음';
  let sourceMarkdown = '';

  $: if ($viewerData) {
    sourceMarkdown = $viewerData.markdown;
  }

  async function openByPath(path: string, name: string) {
    loading = true;
    error = '';
    try {
      viewerData.set(await openFile(path));
      activeFileName = name;
    } catch (caught) {
      error = caught instanceof Error ? caught.message : '파일을 열지 못했습니다.';
    } finally {
      loading = false;
    }
  }

  /** DropZone paths 이벤트 (Tauri dialog) */
  function handlePaths(paths: string[]) {
    const first = paths[0];
    if (first) openByPath(first, first.split('/').pop() ?? first);
  }

  /** DropZone files 이벤트 (브라우저 fallback) */
  async function handleFiles(files: File[]) {
    const first = files[0];
    if (!first) return;

    loading = true;
    error = '';
    try {
      const markdown = await first.text();
      const html = await markdownToHtml(markdown);
      viewerData.set({
        markdown,
        html,
        metadata: {
          format: first.name.split('.').pop() ?? 'markdown',
          title: first.name
        }
      } satisfies ViewerData);
      activeFileName = first.name;
    } catch (caught) {
      error = caught instanceof Error ? caught.message : '파일을 열지 못했습니다.';
    } finally {
      loading = false;
    }
  }

  async function handleSourceInput(event: Event) {
    const value = (event.currentTarget as HTMLTextAreaElement).value;
    sourceMarkdown = value;

    if ($viewerData) {
      viewerData.set({
        ...$viewerData,
        markdown: value,
        html: await markdownToHtml(value)
      });
    }
  }
</script>

<div class="viewer-page">
  <div class="viewer-header">
    <div class="header-left">
      <h2 class="file-name">{activeFileName}</h2>
      <p class="file-desc">렌더, 나란히, 소스 보기 모드를 전환할 수 있습니다.</p>
    </div>
    <div class="header-right">
      <ViewerToggle mode={$viewerMode} on:change={(event) => setViewerMode(event.detail)} />
    </div>
  </div>

  {#if !$viewerData}
    <DropZone
      title="문서를 열어 미리보기"
      description="파일을 드롭하면 마지막 뷰 모드가 자동으로 기억됩니다."
      on:paths={(event) => handlePaths(event.detail.paths)}
      on:files={(event) => handleFiles(event.detail.files)}
    />
  {:else}
    <div class="viewer-body" class:split={$viewerMode === 'split'}>
      {#if error}
        <div class="error-bar">{error}</div>
      {/if}

      {#if $viewerMode !== 'source'}
        <div class="pane-wrapper">
          {#if $viewerMode === 'split'}
            <div class="pane-badge">렌더링</div>
          {/if}
          <article class="rendered-pane">
            {@html $viewerData.html}
          </article>
        </div>
      {/if}

      {#if $viewerMode !== 'render'}
        <div class="pane-wrapper source-wrapper">
          {#if $viewerMode === 'split'}
            <div class="pane-badge source-badge">Markdown 소스</div>
          {/if}
          <textarea
            class="source-pane"
            bind:value={sourceMarkdown}
            on:input={handleSourceInput}
            spellcheck="false"
          ></textarea>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .viewer-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 1400px;
    margin: 0 auto;
  }

  .viewer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-5);
    border-radius: var(--radius-card);
    background: var(--color-bg-card);
    box-shadow: var(--card-shadow);
    border: 1px solid var(--color-separator-non-opaque);
  }

  .file-name {
    margin: 0;
    font-size: var(--text-headline);
    font-weight: 600;
    color: var(--color-label-primary);
  }

  .file-desc {
    margin: var(--space-1) 0 0;
    font-size: var(--text-caption1-size);
    color: var(--color-label-tertiary);
  }

  .header-right {
    flex-shrink: 0;
  }

  .pane-wrapper {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .source-wrapper {
    border-left: 1px solid var(--color-separator-non-opaque);
  }

  .pane-badge {
    flex-shrink: 0;
    padding: var(--space-1) var(--space-3);
    font-size: var(--text-caption2-size);
    font-weight: 600;
    letter-spacing: 0.03em;
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 8%, transparent);
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  .source-badge {
    color: var(--color-success);
    background: color-mix(in srgb, var(--color-success) 8%, transparent);
  }

  .viewer-body {
    display: grid;
    grid-template-columns: 1fr;
    min-height: 500px;
    border-radius: var(--radius-card);
    background: var(--color-bg-card);
    box-shadow: var(--card-shadow);
    border: 1px solid var(--color-separator-non-opaque);
    overflow: hidden;
  }

  .viewer-body.split {
    grid-template-columns: 1fr 1fr;
  }

  .error-bar {
    padding: var(--space-2) var(--space-4);
    background: color-mix(in srgb, var(--color-error) 10%, transparent);
    color: var(--color-error);
    font-size: var(--text-footnote-size);
  }

  .rendered-pane {
    padding: var(--space-6);
    overflow: auto;
    color: var(--color-label-primary);
    line-height: 1.7;
  }

  .rendered-pane :global(h1),
  .rendered-pane :global(h2),
  .rendered-pane :global(h3) {
    margin-top: 0;
    color: var(--color-label-primary);
  }

  /* 테이블 렌더링 */
  .rendered-pane :global(table) {
    width: 100%;
    border-collapse: collapse;
    margin: var(--space-4) 0;
    font-size: var(--text-footnote-size);
  }

  .rendered-pane :global(th),
  .rendered-pane :global(td) {
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-separator);
    text-align: left;
  }

  .rendered-pane :global(th) {
    font-weight: 600;
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
  }

  .rendered-pane :global(td) {
    color: var(--color-label-secondary);
  }

  .rendered-pane :global(tr:hover td) {
    background: var(--color-fill-quaternary);
  }

  /* 코드 블록 */
  .rendered-pane :global(pre) {
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    background: var(--color-fill-quaternary);
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: var(--text-caption1-size);
    line-height: 1.5;
  }

  .rendered-pane :global(code) {
    font-family: var(--font-mono);
    font-size: 0.9em;
  }

  .rendered-pane :global(:not(pre) > code) {
    padding: 1px var(--space-1);
    border-radius: var(--radius-xs);
    background: var(--color-fill-quaternary);
  }

  /* 리스트 */
  .rendered-pane :global(ul),
  .rendered-pane :global(ol) {
    padding-left: var(--space-5);
    margin: var(--space-2) 0;
  }

  .rendered-pane :global(li) {
    margin: var(--space-1) 0;
  }

  /* 구분선 */
  .rendered-pane :global(hr) {
    border: none;
    border-top: 1px solid var(--color-separator);
    margin: var(--space-5) 0;
  }

  /* 인용 */
  .rendered-pane :global(blockquote) {
    margin: var(--space-3) 0;
    padding: var(--space-2) var(--space-4);
    border-left: 3px solid var(--color-accent);
    color: var(--color-label-secondary);
    background: var(--color-fill-quaternary);
    border-radius: 0 var(--radius-xs) var(--radius-xs) 0;
  }

  /* 이미지 */
  .rendered-pane :global(img) {
    max-width: 100%;
    border-radius: var(--radius-sm);
  }

  /* 체크박스 (task list) */
  .rendered-pane :global(input[type="checkbox"]) {
    margin-right: var(--space-1);
    accent-color: var(--color-accent);
  }

  /* 단락 간격 */
  .rendered-pane :global(p) {
    margin: var(--space-2) 0;
  }

  /* 링크 */
  .rendered-pane :global(a) {
    color: var(--color-accent);
    text-decoration: none;
  }

  .rendered-pane :global(a:hover) {
    text-decoration: underline;
  }

  /* 취소선 */
  .rendered-pane :global(del) {
    color: var(--color-label-tertiary);
  }

  /* 강조 */
  .rendered-pane :global(strong) {
    font-weight: 600;
    color: var(--color-label-primary);
  }

  /* heading 간격 */
  .rendered-pane :global(h1) { font-size: var(--text-title1); margin: var(--space-6) 0 var(--space-3); }
  .rendered-pane :global(h2) { font-size: var(--text-title2); margin: var(--space-5) 0 var(--space-2); }
  .rendered-pane :global(h3) { font-size: var(--text-title3); margin: var(--space-4) 0 var(--space-2); }
  .rendered-pane :global(h4) { font-size: var(--text-headline); margin: var(--space-3) 0 var(--space-1); }
  .rendered-pane :global(h5),
  .rendered-pane :global(h6) { font-size: var(--text-subheadline); margin: var(--space-3) 0 var(--space-1); }

  .source-pane {
    width: 100%;
    flex: 1;
    padding: var(--space-5);
    border: 0;
    border-radius: 0;
    background: var(--color-fill-quaternary);
    font-family: var(--font-mono);
    font-size: var(--text-footnote-size);
    color: var(--color-label-primary);
    line-height: 1.6;
    resize: none;
    outline: none;
  }

  @media (max-width: 960px) {
    .viewer-body.split {
      grid-template-columns: 1fr;
    }

    .source-wrapper {
      border-left: 0;
      border-top: 1px solid var(--color-separator-non-opaque);
    }
  }
</style>
