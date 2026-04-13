<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { AppMode } from '$lib/types';

  const dispatch = createEventDispatcher<{ action: AppMode }>();

  const cards: { mode: AppMode; title: string; copy: string; tone: string; svg: string }[] = [
    {
      mode: 'convert',
      title: '문서 → Markdown',
      copy: 'HWP, PDF, DOCX를 Markdown으로 변환',
      tone: 'blue',
      svg: '<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" stroke="currentColor" stroke-width="1.5" fill="none"/><polyline points="14 2 14 8 20 8" stroke="currentColor" stroke-width="1.5" fill="none"/><line x1="16" y1="13" x2="8" y2="13" stroke="currentColor" stroke-width="1.5"/><line x1="16" y1="17" x2="8" y2="17" stroke="currentColor" stroke-width="1.5"/>'
    },
    {
      mode: 'export',
      title: 'MD 변환',
      copy: 'Markdown을 DOCX, PDF, HWPX로 변환',
      tone: 'green',
      svg: '<path d="M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2M7 10l5 5 5-5M12 15V3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>'
    },
    {
      mode: 'batch',
      title: '대량 변환',
      copy: '폴더 단위로 여러 문서를 한 번에 처리',
      tone: 'orange',
      svg: '<rect x="3" y="3" width="7" height="7" rx="1" stroke="currentColor" stroke-width="1.5" fill="none"/><rect x="14" y="3" width="7" height="7" rx="1" stroke="currentColor" stroke-width="1.5" fill="none"/><rect x="3" y="14" width="7" height="7" rx="1" stroke="currentColor" stroke-width="1.5" fill="none"/><rect x="14" y="14" width="7" height="7" rx="1" stroke="currentColor" stroke-width="1.5" fill="none"/>'
    }
  ];
</script>

<div class="cards">
  {#each cards as card}
    <button
      type="button"
      class="card {card.tone}"
      aria-label="{card.title}: {card.copy}"
      on:click={() => dispatch('action', card.mode)}
    >
      <div class="card-icon">
        <svg width="20" height="20" viewBox="0 0 24 24">{@html card.svg}</svg>
      </div>
      <div class="card-text">
        <strong>{card.title}</strong>
        <span>{card.copy}</span>
      </div>
    </button>
  {/each}
</div>

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-3);
  }

  .card {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-4);
    text-align: left;
    border-radius: var(--radius-card);
    border: 1px solid var(--color-separator-non-opaque);
    background: var(--color-bg-card);
    box-shadow: var(--card-shadow);
    cursor: pointer;
    transition:
      background var(--duration-fast) var(--ease-default),
      border-color var(--duration-fast) var(--ease-default),
      box-shadow var(--duration-fast) var(--ease-default);
  }

  .card:hover {
    background: var(--color-bg-card-hover);
    border-color: var(--color-separator);
    box-shadow: var(--card-shadow-hover);
  }

  .card-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
  }

  .blue .card-icon {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
  }

  .green .card-icon {
    color: var(--color-success);
    background: color-mix(in srgb, var(--color-success) 12%, transparent);
  }

  .orange .card-icon {
    color: var(--color-warning);
    background: color-mix(in srgb, var(--color-warning) 12%, transparent);
  }

  .card-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .card-text strong {
    font-size: var(--text-subheadline-size);
    font-weight: 600;
    color: var(--color-label-primary);
  }

  .card-text span {
    font-size: var(--text-caption1-size);
    color: var(--color-label-secondary);
    line-height: 1.4;
  }

  @media (max-width: 960px) {
    .cards {
      grid-template-columns: 1fr;
    }
  }
</style>
