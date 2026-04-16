<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import {
    rhwpListParagraphs,
    rhwpSaveWithEdits,
    pickFileWithDialog,
    type RhwpParagraph,
    type RhwpSaveSummary,
  } from '$lib/utils/ipc';

  export let open = false;
  export let sourcePath: string | null = null;

  const dispatch = createEventDispatcher<{
    close: void;
    saved: RhwpSaveSummary;
  }>();

  interface ParagraphRow extends RhwpParagraph {
    edited: string;
  }

  let rows: ParagraphRow[] = [];
  let loading = false;
  let saving = false;
  let error = '';
  let lastSave: RhwpSaveSummary | null = null;

  $: if (open && sourcePath && rows.length === 0 && !loading) {
    loadParagraphs();
  }

  async function loadParagraphs() {
    if (!sourcePath) return;
    loading = true;
    error = '';
    try {
      const list = await rhwpListParagraphs(sourcePath);
      rows = list.map((p) => ({ ...p, edited: p.text }));
    } catch (e) {
      error = e instanceof Error ? e.message : '문단 목록을 불러오지 못했습니다.';
    } finally {
      loading = false;
    }
  }

  function isDirty(r: ParagraphRow): boolean {
    return r.edited !== r.text;
  }

  $: dirtyCount = rows.filter(isDirty).length;

  async function save() {
    if (!sourcePath) return;
    const edits = rows
      .filter(isDirty)
      .map((r) => ({ section: r.section, index: r.index, newText: r.edited }));

    // Suggest save-as path alongside the source.
    const suggested = sourcePath.replace(/(\.hwpx?)$/i, '-edited$1');
    const picked = await pickFileWithDialog({
      title: '저장할 위치 선택',
      multiple: false,
      filters: [{ name: 'HWP 문서', extensions: ['hwp', 'hwpx'] }],
    });
    let target: string | null = null;
    if (typeof picked === 'string') target = picked;
    else if (Array.isArray(picked) && picked.length > 0) target = picked[0];
    else target = suggested; // fallback — Tauri dialog returned null (user canceled)

    if (!target) return;

    saving = true;
    error = '';
    try {
      const summary = await rhwpSaveWithEdits(sourcePath, target, edits);
      lastSave = summary;
      dispatch('saved', summary);
      // Commit the edits locally so the UI no longer shows them as dirty.
      rows = rows.map((r) =>
        isDirty(r) ? { ...r, text: r.edited } : r
      );
    } catch (e) {
      error = e instanceof Error ? e.message : 'HWP 저장 실패';
    } finally {
      saving = false;
    }
  }

  function revert(i: number) {
    rows[i] = { ...rows[i], edited: rows[i].text };
    rows = rows;
  }

  function close() {
    dispatch('close');
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && !saving) close();
  }

  onMount(() => {
    if (open && sourcePath && rows.length === 0) loadParagraphs();
  });
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
      aria-labelledby="hwp-edit-title"
      tabindex="-1"
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <header>
        <div>
          <h3 id="hwp-edit-title">HWP 편집</h3>
          <p class="subtitle">
            rhwp로 직접 수정하고 새 HWP 파일로 저장합니다. 원본은 보존됩니다.
          </p>
        </div>
        <div class="header-actions">
          <span class="counter">변경 {dirtyCount}</span>
          <button
            type="button"
            class="primary"
            disabled={saving || dirtyCount === 0}
            on:click={save}
          >
            {#if saving}저장 중…{:else}HWP로 저장{/if}
          </button>
          <button type="button" class="close" on:click={close} aria-label="닫기">×</button>
        </div>
      </header>

      {#if error}
        <div class="error-bar">{error}</div>
      {/if}

      {#if lastSave}
        <div class="success-bar">
          저장됨 · {lastSave.paragraphsEdited}개 문단 · {(lastSave.outputBytes / 1024).toFixed(1)}KB
          → {lastSave.outputPath}
        </div>
      {/if}

      {#if loading}
        <div class="message">문단 목록을 불러오는 중…</div>
      {:else if rows.length === 0}
        <div class="message">편집할 문단이 없습니다.</div>
      {:else}
        <div class="list">
          {#each rows as row, i (`${row.section}-${row.index}`)}
            <article class="row" class:dirty={isDirty(row)}>
              <div class="addr">구역 {row.section + 1} · 문단 {row.index + 1}</div>
              <textarea
                bind:value={row.edited}
                rows="2"
                spellcheck="false"
                aria-label="구역 {row.section + 1} 문단 {row.index + 1}"
              ></textarea>
              {#if isDirty(row)}
                <button
                  type="button"
                  class="revert"
                  on:click={() => revert(i)}
                  title="이 문단만 원래대로"
                >
                  되돌리기
                </button>
              {/if}
            </article>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, black 45%, transparent);
    display: flex;
    justify-content: flex-end;
    z-index: 100;
  }

  .panel {
    width: min(640px, 98vw);
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
    max-width: 42ch;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .counter {
    font-size: var(--text-caption1-size);
    color: var(--color-label-secondary);
    font-variant-numeric: tabular-nums;
    padding: 2px 10px;
    border-radius: 999px;
    background: var(--color-fill-quaternary);
  }

  .primary {
    padding: 6px 14px;
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

  .error-bar {
    padding: var(--space-2) var(--space-5);
    background: color-mix(in srgb, var(--color-error) 12%, transparent);
    color: var(--color-error);
    font-size: var(--text-footnote-size);
  }

  .success-bar {
    padding: var(--space-2) var(--space-5);
    background: color-mix(in srgb, var(--color-success) 12%, transparent);
    color: var(--color-success);
    font-size: var(--text-footnote-size);
    font-variant-numeric: tabular-nums;
  }

  .message {
    padding: var(--space-6) var(--space-5);
    text-align: center;
    color: var(--color-label-tertiary);
    font-size: var(--text-footnote-size);
  }

  .list {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-2) 0;
  }

  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-5);
    border-bottom: 1px solid var(--color-separator-non-opaque);
    align-items: start;
  }

  .row.dirty {
    background: color-mix(in srgb, var(--color-accent) 6%, transparent);
  }

  .addr {
    grid-column: 1 / -1;
    font-size: var(--text-caption2-size);
    font-weight: 600;
    letter-spacing: 0.03em;
    color: var(--color-label-tertiary);
    text-transform: uppercase;
  }

  textarea {
    width: 100%;
    padding: var(--space-2);
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: var(--radius-xs);
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
    font-family: inherit;
    font-size: var(--text-footnote-size);
    line-height: 1.5;
    resize: vertical;
  }

  .row.dirty textarea {
    border-color: var(--color-accent);
    background: var(--color-bg-card);
  }

  .revert {
    grid-column: 2;
    align-self: center;
    padding: 2px 10px;
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: 999px;
    background: transparent;
    color: var(--color-label-secondary);
    font-size: var(--text-caption1-size);
    cursor: pointer;
  }

  .revert:hover {
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
  }
</style>
