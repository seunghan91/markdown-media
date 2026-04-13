<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { HistoryEntry } from '$lib/types';

  export let entries: HistoryEntry[] = [];
  export let selectedId: number | null = null;
  export let title = '최근 변환';
  export let pageSize = 5;

  const dispatch = createEventDispatcher<{
    select: HistoryEntry;
    delete: HistoryEntry;
  }>();

  let visibleCount = pageSize;

  $: visibleEntries = entries.slice(0, visibleCount);
  $: hasMore = entries.length > visibleCount;

  function showMore() {
    visibleCount += pageSize;
  }

  function handleDelete(entry: HistoryEntry) {
    dispatch('delete', entry);
    // 로컬에서 즉시 제거 (부모가 store를 업데이트하지 않더라도 UX 반영)
  }
</script>

<section class="file-list">
  <div class="header">
    <h3 class="list-title">{title}</h3>
    <span class="list-count">{entries.length}건</span>
  </div>

  <div class="rows">
    {#if entries.length === 0}
      <p class="empty">아직 기록이 없습니다.</p>
    {:else}
      {#each visibleEntries as entry (entry.id)}
        <div
          class="row"
          class:selected={entry.id === selectedId}
        >
          <button
            type="button"
            class="row-content"
            aria-pressed={entry.id === selectedId}
            on:click={() => dispatch('select', entry)}
          >
            <div class="row-left">
              <strong class="row-name">{entry.fileName}</strong>
              <span class="row-date">{entry.createdAt.split('T')[0]}</span>
            </div>

            <div class="row-right">
              <span class="format-tag">{entry.outputFormat.toUpperCase()}</span>
              <span class="status-tag" class:success={entry.status === 'success'} class:failed={entry.status !== 'success'}>
                {entry.status === 'success' ? '완료' : '실패'}
              </span>
            </div>
          </button>

          <button
            type="button"
            class="delete-btn"
            aria-label="{entry.fileName} 삭제"
            on:click|stopPropagation={() => handleDelete(entry)}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
              <line x1="18" y1="6" x2="6" y2="18" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
              <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
            </svg>
          </button>
        </div>
      {/each}

      {#if hasMore}
        <button type="button" class="more-btn" on:click={showMore}>
          더보기 ({entries.length - visibleCount}건 남음)
        </button>
      {/if}
    {/if}
  </div>
</section>

<style>
  .file-list {
    padding: var(--space-4);
    border-radius: var(--radius-card);
    border: 1px solid var(--color-separator-non-opaque);
    background: var(--color-bg-card);
    box-shadow: var(--card-shadow);
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--space-3);
  }

  .list-title {
    margin: 0;
    font-size: var(--text-subheadline-size);
    font-weight: 600;
    color: var(--color-label-primary);
  }

  .list-count {
    font-size: var(--text-caption1-size);
    color: var(--color-label-tertiary);
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .row {
    display: flex;
    align-items: center;
    border-radius: var(--radius-sm);
    transition: background var(--duration-fast) var(--ease-default);
  }

  .row:hover {
    background: var(--color-fill-quaternary);
  }

  .row.selected {
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
  }

  .row-content {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex: 1;
    gap: var(--space-3);
    min-height: 44px;
    padding: var(--space-2) var(--space-3);
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }

  .row-left {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .row-name {
    font-size: var(--text-footnote-size);
    font-weight: 500;
    color: var(--color-label-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-date {
    font-size: var(--text-caption2-size);
    color: var(--color-label-tertiary);
  }

  .row-right {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
  }

  .format-tag {
    font-size: var(--text-caption2-size);
    font-weight: 600;
    padding: 1px var(--space-1);
    border-radius: var(--radius-xs);
    background: var(--color-fill-quaternary);
    color: var(--color-label-secondary);
  }

  .status-tag {
    font-size: var(--text-caption2-size);
    font-weight: 600;
  }

  .status-tag.success { color: var(--color-success); }
  .status-tag.failed { color: var(--color-error); }

  .delete-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    min-height: 28px;
    min-width: 28px;
    border: 0;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--color-label-tertiary);
    cursor: pointer;
    opacity: 0;
    margin-right: var(--space-1);
    transition:
      opacity var(--duration-fast) var(--ease-default),
      background var(--duration-fast) var(--ease-default),
      color var(--duration-fast) var(--ease-default);
  }

  .row:hover .delete-btn {
    opacity: 1;
  }

  .delete-btn:hover {
    background: color-mix(in srgb, var(--color-error) 12%, transparent);
    color: var(--color-error);
  }

  .more-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    padding: var(--space-2);
    margin-top: var(--space-1);
    border: 0;
    border-radius: var(--radius-sm);
    background: var(--color-fill-quaternary);
    color: var(--color-accent);
    font-size: var(--text-caption1-size);
    font-weight: 500;
    cursor: pointer;
    min-height: auto;
    min-width: auto;
    transition: background var(--duration-fast) var(--ease-default);
  }

  .more-btn:hover {
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  }

  .empty {
    margin: 0;
    padding: var(--space-4) 0;
    text-align: center;
    font-size: var(--text-footnote-size);
    color: var(--color-label-tertiary);
  }
</style>
