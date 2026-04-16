<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { DocumentStats } from '$lib/utils/markdownStats';

  export let stats: DocumentStats;
  export let open = false;

  const dispatch = createEventDispatcher<{ close: void }>();

  const rows: { label: string; value: keyof DocumentStats; fmt?: (n: number) => string }[] = [
    { label: '문자 수', value: 'charCount', fmt: (n) => n.toLocaleString() },
    { label: '단어 수 (어절)', value: 'wordCount', fmt: (n) => n.toLocaleString() },
    { label: '문단', value: 'paragraphCount' },
    { label: '헤딩', value: 'headingCount' },
    { label: '표', value: 'tableCount' },
    { label: '이미지', value: 'imageCount' },
    { label: '강조 (<mark>)', value: 'emphasisCount' },
    { label: '취소선', value: 'strikeoutCount' },
    { label: '체크리스트', value: 'taskCount' },
  ];

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
    on:click={close}
    on:keydown={(e) => e.key === 'Enter' && close()}
    role="presentation"
  >
    <div
      class="panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="stats-title"
      tabindex="-1"
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <header>
        <h3 id="stats-title">문서 통계</h3>
        <button type="button" class="close" on:click={close} aria-label="닫기">×</button>
      </header>
      <dl>
        {#each rows as row}
          <div class="row">
            <dt>{row.label}</dt>
            <dd>{row.fmt ? row.fmt(stats[row.value]) : stats[row.value]}</dd>
          </div>
        {/each}
      </dl>
      <footer>
        <p class="note">
          수치는 변환된 마크다운을 기준으로 계산됩니다. 원본 HWP의 페이지 수와 다를 수 있습니다.
        </p>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, black 40%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .panel {
    width: min(420px, 90vw);
    max-height: 80vh;
    overflow: auto;
    background: var(--color-bg-card);
    border-radius: var(--radius-card);
    box-shadow: var(--card-shadow-hover);
    border: 1px solid var(--color-separator-non-opaque);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-4) var(--space-5);
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  header h3 {
    margin: 0;
    font-size: var(--text-headline);
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

  .close:hover {
    background: var(--color-fill-quaternary);
    color: var(--color-label-primary);
  }

  dl {
    margin: 0;
    padding: var(--space-3) var(--space-5);
  }

  .row {
    display: flex;
    justify-content: space-between;
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  .row:last-child {
    border-bottom: 0;
  }

  dt {
    color: var(--color-label-secondary);
    font-size: var(--text-footnote-size);
  }

  dd {
    margin: 0;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    color: var(--color-label-primary);
    font-weight: 600;
  }

  footer {
    padding: var(--space-3) var(--space-5) var(--space-4);
  }

  .note {
    margin: 0;
    font-size: var(--text-caption1-size);
    color: var(--color-label-tertiary);
  }
</style>
