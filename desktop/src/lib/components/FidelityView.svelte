<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { createEditor, type RhwpEditor } from '@rhwp/editor';

  /**
   * Raw HWP / HWPX bytes. When this changes, the editor reloads.
   */
  export let bytes: ArrayBuffer | Uint8Array | null = null;
  export let fileName: string = 'document.hwp';

  let container: HTMLDivElement;
  let editor: RhwpEditor | null = null;
  let status: 'idle' | 'loading' | 'ready' | 'error' = 'idle';
  let errorMessage = '';
  let pageCount = 0;

  async function ensureEditor() {
    if (editor || !container) return;
    status = 'loading';
    try {
      editor = await createEditor(container, {
        width: '100%',
        height: '100%',
      });
    } catch (err) {
      status = 'error';
      errorMessage = err instanceof Error ? err.message : '에디터 로드 실패';
      throw err;
    }
  }

  async function loadBytes(payload: ArrayBuffer | Uint8Array, name: string) {
    if (!editor) await ensureEditor();
    if (!editor) return;
    status = 'loading';
    errorMessage = '';
    try {
      const result = await editor.loadFile(payload, name);
      pageCount = result.pageCount;
      status = 'ready';
    } catch (err) {
      status = 'error';
      errorMessage = err instanceof Error ? err.message : '문서 로드 실패';
    }
  }

  $: if (bytes && container) {
    loadBytes(bytes, fileName);
  }

  onMount(() => {
    // Lazy init: editor is only created when bytes arrive. Mount keeps the
    // container ready so the $: block above fires on first assignment.
  });

  onDestroy(() => {
    if (editor) {
      editor.destroy();
      editor = null;
    }
  });
</script>

<div class="wrap">
  {#if status === 'loading'}
    <div class="overlay">원본 뷰어 로딩 중…</div>
  {:else if status === 'error'}
    <div class="overlay error">
      <strong>원본 렌더러를 불러오지 못했습니다.</strong>
      <p>{errorMessage}</p>
      <p class="hint">네트워크 또는 edwardkim.github.io/rhwp 접근 가능 여부를 확인해주세요.</p>
    </div>
  {/if}
  {#if !bytes && status !== 'error'}
    <div class="overlay idle">
      <strong>원본 문서 바이트가 없습니다.</strong>
      <p class="hint">파일을 다시 선택하거나 MDM이 원본 경로를 보유한 상태로 열어주세요.</p>
    </div>
  {/if}
  <div class="embed" bind:this={container}></div>
  {#if status === 'ready'}
    <div class="footer">
      <span class="badge">rhwp 원본 렌더링</span>
      <span class="meta">페이지 {pageCount}</span>
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 500px;
    display: flex;
    flex-direction: column;
  }

  .embed {
    flex: 1;
    min-height: 0;
    background: var(--color-bg-card);
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    padding: var(--space-5);
    background: color-mix(in srgb, var(--color-bg-card) 95%, transparent);
    text-align: center;
    z-index: 10;
  }

  .overlay strong {
    color: var(--color-label-primary);
    font-size: var(--text-subheadline-size);
  }

  .overlay p {
    margin: 0;
    color: var(--color-label-secondary);
    font-size: var(--text-footnote-size);
  }

  .overlay.error strong {
    color: var(--color-error);
  }

  .hint {
    color: var(--color-label-tertiary);
    max-width: 32ch;
  }

  .footer {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-4);
    border-top: 1px solid var(--color-separator-non-opaque);
    background: var(--color-fill-quaternary);
  }

  .badge {
    padding: 2px 10px;
    border-radius: 999px;
    font-size: var(--text-caption2-size);
    font-weight: 600;
    letter-spacing: 0.04em;
    color: white;
    background: var(--color-accent);
    text-transform: uppercase;
  }

  .meta {
    font-size: var(--text-caption1-size);
    color: var(--color-label-secondary);
    font-variant-numeric: tabular-nums;
  }
</style>
