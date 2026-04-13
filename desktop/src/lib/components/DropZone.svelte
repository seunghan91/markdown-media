<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { pickFileWithDialog } from '$lib/utils/ipc';

  export let title = '문서를 여기에 놓으세요';
  export let description = '지원 포맷: HWP · HWPX · PDF · DOCX · Markdown';
  export let buttonLabel = '파일 선택';
  export let multiple = false;
  export let directory = false;

  const dispatch = createEventDispatcher<{
    files: { files: File[] };
    paths: { paths: string[] };
  }>();

  let isDragging = false;
  let inputEl: HTMLInputElement;

  function emitFiles(files: FileList | null) {
    dispatch('files', { files: files ? Array.from(files) : [] });
  }

  function handleDrop(event: DragEvent) {
    event.preventDefault();
    isDragging = false;
    emitFiles(event.dataTransfer?.files ?? null);
  }

  async function handleClick() {
    const dialogResult = await pickFileWithDialog({
      title,
      multiple,
      directory,
    });

    // Tauri dialog가 열렸고 파일을 선택한 경우
    if (dialogResult) {
      const paths = Array.isArray(dialogResult) ? dialogResult : [dialogResult];
      dispatch('paths', { paths });
      return;
    }

    // Tauri dialog가 열렸지만 취소한 경우 → 아무것도 안 함
    // pickFileWithDialog는 Tauri 환경이 아닐 때만 null 반환
    // Tauri 환경에서 취소 시에도 null이므로 여기서 끝
    if (typeof window !== 'undefined' && typeof (window as any).__TAURI_INTERNALS__ !== 'undefined') {
      return;
    }

    // 브라우저 환경에서만 HTML input fallback
    inputEl?.click();
  }
</script>

<div
  class:hovered={isDragging}
  class="drop-zone"
  role="button"
  tabindex="0"
  aria-label={title}
  on:dragenter|preventDefault={() => (isDragging = true)}
  on:dragover|preventDefault={() => (isDragging = true)}
  on:dragleave|preventDefault={() => (isDragging = false)}
  on:drop={handleDrop}
  on:click={handleClick}
  on:keydown={(e) => e.key === 'Enter' && handleClick()}
>
  <div class="drop-icon">
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
      <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
      <polyline points="7 10 12 15 17 10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
      <line x1="12" y1="15" x2="12" y2="3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
  </div>

  <div class="drop-text">
    <h3>{title}</h3>
    <p>{description}</p>
  </div>

  <div class="drop-actions">
    <button class="drop-btn" type="button" on:click|stopPropagation={handleClick}>
      {buttonLabel}
    </button>
    <span class="drop-formats">HWP · PDF · DOCX</span>
  </div>

  <input
    bind:this={inputEl}
    class="native-input"
    type="file"
    {multiple}
    webkitdirectory={directory}
    on:change={(event) => emitFiles((event.currentTarget as HTMLInputElement).files)}
  />
</div>

<style>
  .drop-zone {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-8) var(--space-6);
    border: 1.5px dashed var(--color-separator);
    border-radius: var(--radius-xl);
    background: var(--color-bg-card);
    box-shadow: var(--card-shadow);
    cursor: pointer;
    transition:
      border-color var(--duration-fast) var(--ease-default),
      background var(--duration-fast) var(--ease-default),
      transform var(--duration-fast) var(--ease-default);
  }

  .drop-zone:hover {
    border-color: var(--color-gray);
    background: var(--color-bg-card-hover);
  }

  .drop-zone.hovered {
    border-color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 8%, transparent);
    transform: scale(1.01);
  }

  .drop-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 52px;
    height: 52px;
    border-radius: var(--radius-lg);
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  }

  .hovered .drop-icon {
    background: color-mix(in srgb, var(--color-accent) 18%, transparent);
  }

  .drop-text {
    text-align: center;
  }

  .drop-text h3 {
    margin: 0;
    font-size: var(--text-callout-size);
    font-weight: 600;
    color: var(--color-label-primary);
  }

  .drop-text p {
    margin: var(--space-1) 0 0;
    font-size: var(--text-footnote-size);
    color: var(--color-label-secondary);
  }

  .drop-actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .drop-btn {
    padding: var(--space-2) var(--space-5);
    border: 0;
    border-radius: var(--radius-full);
    background: var(--color-accent);
    color: var(--color-bg-primary);
    font-size: var(--text-footnote-size);
    font-weight: 600;
    cursor: pointer;
    min-height: auto;
    min-width: auto;
    transition: opacity var(--duration-fast) var(--ease-default);
  }

  .drop-btn:hover {
    opacity: 0.85;
  }

  .drop-formats {
    font-size: var(--text-caption1-size);
    color: var(--color-label-tertiary);
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-full);
    background: var(--color-fill-quaternary);
  }

  .native-input {
    display: none;
  }
</style>
