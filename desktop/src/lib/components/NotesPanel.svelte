<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { SidecarNotes } from '$lib/utils/notesStore';
  import { addNote, deleteNote, exportSidecar } from '$lib/utils/notesStore';

  export let open = false;
  export let documentTitle = '';
  export let sidecar: SidecarNotes;

  const dispatch = createEventDispatcher<{
    close: void;
    change: SidecarNotes;
  }>();

  let draft = '';
  let draftLine = '';

  function close() {
    dispatch('close');
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
  }

  function submit() {
    const lineNum = draftLine.trim() ? Number.parseInt(draftLine, 10) : undefined;
    const line = Number.isFinite(lineNum) ? lineNum : undefined;
    const next = addNote(sidecar, draft, line);
    dispatch('change', next);
    draft = '';
    draftLine = '';
  }

  function remove(id: string) {
    dispatch('change', deleteNote(sidecar, id));
  }

  function downloadSidecar() {
    const json = exportSidecar(sidecar);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    const base = documentTitle.replace(/\.[^.]+$/, '') || 'document';
    a.href = url;
    a.download = `${base}.mdm.notes.json`;
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

<svelte:window on:keydown={onKey} />

{#if open}
  <div
    class="backdrop"
    role="presentation"
    on:click={close}
    on:keydown={(e) => e.key === 'Enter' && close()}
  >
    <div
      class="panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="notes-title"
      tabindex="-1"
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <header>
        <div>
          <h3 id="notes-title">메모</h3>
          <p class="subtitle">원본 문서는 수정되지 않습니다. 메모는 사이드카로 저장됩니다.</p>
        </div>
        <div class="header-actions">
          <button type="button" class="ghost" on:click={downloadSidecar} title="JSON으로 저장">
            내보내기
          </button>
          <button type="button" class="close" on:click={close} aria-label="닫기">×</button>
        </div>
      </header>

      <form class="composer" on:submit|preventDefault={submit}>
        <div class="composer-row">
          <input
            type="number"
            class="line-input"
            placeholder="줄 #"
            min="0"
            bind:value={draftLine}
            aria-label="줄 번호 (선택)"
          />
          <textarea
            class="text-input"
            placeholder="이 문단에 대한 메모…"
            rows="2"
            bind:value={draft}
          ></textarea>
        </div>
        <div class="composer-actions">
          <button type="submit" class="primary" disabled={!draft.trim()}>추가</button>
        </div>
      </form>

      {#if sidecar.notes.length === 0}
        <div class="empty">아직 메모가 없습니다.</div>
      {:else}
        <ul class="list">
          {#each sidecar.notes as note (note.id)}
            <li>
              <div class="note-head">
                <span class="line-tag">
                  {note.line !== undefined ? `줄 ${note.line + 1}` : '전체'}
                </span>
                <time>{new Date(note.createdAt).toLocaleString()}</time>
                <button
                  type="button"
                  class="delete"
                  aria-label="메모 삭제"
                  on:click={() => remove(note.id)}
                >
                  삭제
                </button>
              </div>
              <p class="note-text">{note.text}</p>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, black 30%, transparent);
    display: flex;
    justify-content: flex-end;
    z-index: 100;
  }

  .panel {
    width: min(460px, 95vw);
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-card);
    border-left: 1px solid var(--color-separator-non-opaque);
    box-shadow: var(--card-shadow-hover);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  header h3 {
    margin: 0;
    font-size: var(--text-headline);
    color: var(--color-label-primary);
  }

  .subtitle {
    margin: 4px 0 0;
    font-size: var(--text-caption1-size);
    color: var(--color-label-tertiary);
    max-width: 28ch;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .ghost {
    padding: 4px 10px;
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: 999px;
    background: transparent;
    color: var(--color-label-secondary);
    font-size: var(--text-caption1-size);
    cursor: pointer;
  }

  .ghost:hover {
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
  }

  .close {
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--color-label-secondary);
    font-size: 20px;
    line-height: 1;
    cursor: pointer;
  }

  .composer {
    padding: var(--space-3) var(--space-5);
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  .composer-row {
    display: flex;
    gap: var(--space-2);
    align-items: stretch;
  }

  .line-input {
    width: 72px;
    padding: var(--space-2);
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: var(--radius-xs);
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
  }

  .text-input {
    flex: 1;
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: var(--radius-xs);
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
    line-height: 1.4;
    resize: vertical;
    min-height: 48px;
  }

  .composer-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: var(--space-2);
  }

  .primary {
    padding: 6px 16px;
    border: 0;
    border-radius: 999px;
    background: var(--color-accent);
    color: white;
    font-size: var(--text-footnote-size);
    cursor: pointer;
  }

  .primary:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .empty {
    padding: var(--space-6) var(--space-5);
    text-align: center;
    color: var(--color-label-tertiary);
    font-size: var(--text-footnote-size);
  }

  .list {
    flex: 1;
    overflow-y: auto;
    margin: 0;
    padding: var(--space-2) 0;
    list-style: none;
  }

  .list li {
    padding: var(--space-3) var(--space-5);
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  .note-head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-caption1-size);
    color: var(--color-label-tertiary);
    margin-bottom: 4px;
  }

  .line-tag {
    padding: 1px 8px;
    border-radius: 999px;
    background: var(--color-fill-quaternary);
    color: var(--color-label-secondary);
    font-variant-numeric: tabular-nums;
  }

  time {
    flex: 1;
  }

  .delete {
    padding: 2px 8px;
    border: 0;
    background: transparent;
    color: var(--color-error);
    cursor: pointer;
    font-size: var(--text-caption1-size);
  }

  .note-text {
    margin: 0;
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
