<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  /**
   * Persistent left-column toolbar. Always visible in the viewer. The
   * primary action is "HWP 편집하기" which switches the viewer into
   * 원본(fidelity) mode — rhwp-studio loaded via `@rhwp/editor` iframe,
   * a full-fledged HWP editor. Disabled unless the current file is
   * .hwp / .hwpx.
   */
  export let sourcePath: string | null = null;

  const dispatch = createEventDispatcher<{ editHwp: void }>();

  $: isHwp = !!sourcePath && /\.(hwp|hwpx)$/i.test(sourcePath);
</script>

<aside class="hwp-sidebar" aria-label="HWP 도구">
  <button
    type="button"
    class="sb-btn primary"
    disabled={!isHwp}
    on:click={() => dispatch('editHwp')}
    title={isHwp
      ? 'rhwp 에디터로 HWP 편집 (원본 모드 전환)'
      : 'HWP / HWPX 파일을 먼저 열어주세요'}
  >
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
      <path d="M12 20h9" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <path
        d="M16.5 3.5a2.121 2.121 0 113 3L7 19l-4 1 1-4L16.5 3.5z"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linejoin="round"
      />
    </svg>
    <span>HWP 편집하기</span>
  </button>

  <p class="hint">
    rhwp 에디터 (원본 모드)로 전환합니다. 저장은 에디터의 <strong>파일 &gt; 저장</strong> 메뉴를
    사용하세요.
  </p>
</aside>

<style>
  .hwp-sidebar {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-3);
    width: 180px;
    min-width: 180px;
    border-right: 1px solid var(--color-separator-non-opaque);
    background: color-mix(in srgb, var(--color-accent) 4%, var(--color-bg-card));
  }

  .sb-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid var(--color-separator-non-opaque);
    border-radius: var(--radius-sm);
    background: var(--color-bg-card);
    color: var(--color-label-primary);
    font-size: var(--text-footnote-size);
    text-align: left;
    cursor: pointer;
    transition: background var(--duration-fast) var(--ease-default);
  }

  .sb-btn svg {
    color: var(--color-label-secondary);
    flex-shrink: 0;
  }

  .sb-btn:hover:not(:disabled) {
    background: var(--color-bg-card-hover);
  }

  .sb-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .sb-btn.primary {
    background: var(--color-accent);
    color: white;
    border-color: var(--color-accent);
    font-weight: 600;
  }

  .sb-btn.primary svg {
    color: white;
  }

  .sb-btn.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-accent) 88%, black);
  }

  .hint {
    margin: 0;
    font-size: var(--text-caption2-size);
    color: var(--color-label-tertiary);
    line-height: 1.45;
  }

  .hint strong {
    color: var(--color-label-secondary);
    font-weight: 600;
  }
</style>
