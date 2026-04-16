<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { DiffResult } from '$lib/utils/markdownDiff';

  export let open = false;
  export let leftTitle = '원본';
  export let rightTitle = '변경';
  export let result: DiffResult;

  const dispatch = createEventDispatcher<{ close: void }>();

  function close() {
    dispatch('close');
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
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
      aria-labelledby="diff-title"
      tabindex="-1"
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <header>
        <h3 id="diff-title">신구 대조</h3>
        <div class="summary">
          <span class="badge badge-equal">같음 {result.stats.equal}</span>
          <span class="badge badge-removed">삭제 {result.stats.removed}</span>
          <span class="badge badge-added">추가 {result.stats.added}</span>
        </div>
        <button type="button" class="close" on:click={close} aria-label="닫기">×</button>
      </header>

      <div class="columns">
        <div class="col-title">{leftTitle}</div>
        <div class="col-title">{rightTitle}</div>
      </div>

      <div class="diff-body">
        {#each result.ops as op, i (i)}
          {#if op.kind === 'equal'}
            <div class="row equal">
              <div class="cell left">{op.left || '\u00A0'}</div>
              <div class="cell right">{op.right || '\u00A0'}</div>
            </div>
          {:else if op.kind === 'delete'}
            <div class="row removed">
              <div class="cell left">{op.left || '\u00A0'}</div>
              <div class="cell right empty">—</div>
            </div>
          {:else}
            <div class="row added">
              <div class="cell left empty">—</div>
              <div class="cell right">{op.right || '\u00A0'}</div>
            </div>
          {/if}
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, black 50%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .panel {
    width: min(1200px, 95vw);
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-card);
    border-radius: var(--radius-card);
    box-shadow: var(--card-shadow-hover);
    border: 1px solid var(--color-separator-non-opaque);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-5);
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  header h3 {
    margin: 0;
    font-size: var(--text-headline);
    color: var(--color-label-primary);
  }

  .summary {
    display: flex;
    gap: var(--space-2);
    flex: 1;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 10px;
    border-radius: 999px;
    font-size: var(--text-caption1-size);
    font-variant-numeric: tabular-nums;
  }

  .badge-equal {
    background: var(--color-fill-quaternary);
    color: var(--color-label-secondary);
  }

  .badge-removed {
    background: color-mix(in srgb, var(--color-error) 15%, transparent);
    color: var(--color-error);
  }

  .badge-added {
    background: color-mix(in srgb, var(--color-success) 15%, transparent);
    color: var(--color-success);
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

  .close:hover {
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
  }

  .columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  .col-title {
    padding: var(--space-2) var(--space-4);
    font-size: var(--text-caption1-size);
    font-weight: 600;
    color: var(--color-label-secondary);
    background: var(--color-fill-quaternary);
  }

  .col-title:first-child {
    border-right: 1px solid var(--color-separator-non-opaque);
  }

  .diff-body {
    overflow: auto;
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--text-caption1-size);
    line-height: 1.6;
  }

  .row {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }

  .cell {
    padding: 2px var(--space-4);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .cell.left {
    border-right: 1px solid var(--color-separator-non-opaque);
  }

  .cell.empty {
    color: var(--color-label-tertiary);
    font-style: italic;
  }

  .row.equal {
    color: var(--color-label-primary);
  }

  .row.removed .cell.left {
    background: color-mix(in srgb, var(--color-error) 12%, transparent);
    color: var(--color-label-primary);
  }

  .row.added .cell.right {
    background: color-mix(in srgb, var(--color-success) 12%, transparent);
    color: var(--color-label-primary);
  }
</style>
