<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { rhwpSaveWithEdits } from '$lib/utils/ipc';

  /**
   * Left-column toolbar that appears only when the active file is HWP /
   * HWPX. Every action here routes through the vendored rhwp crate —
   * separate from the right-hand header bar, which operates on the MDM
   * extracted markdown.
   */
  export let sourcePath: string | null = null;

  const dispatch = createEventDispatcher<{
    edit: void;
    saved: { bytes: number; path: string };
  }>();

  let roundtripping = false;
  let feedback: string | null = null;

  async function roundTripCopy() {
    if (!sourcePath) return;
    const target = sourcePath.replace(/(\.hwpx?)$/i, '-copy$1');
    roundtripping = true;
    feedback = null;
    try {
      const summary = await rhwpSaveWithEdits(sourcePath, target, []);
      feedback = `복사본 저장됨 · ${(summary.outputBytes / 1024).toFixed(1)}KB`;
      dispatch('saved', { bytes: summary.outputBytes, path: summary.outputPath });
    } catch (e) {
      feedback = e instanceof Error ? e.message : 'HWP 복사본 저장 실패';
    } finally {
      roundtripping = false;
      setTimeout(() => (feedback = null), 2500);
    }
  }
</script>

<aside class="hwp-sidebar" aria-label="HWP 전용 도구">
  <div class="head">
    <div class="badge">HWP</div>
    <span class="caption">rhwp 기반</span>
  </div>

  <button
    type="button"
    class="sb-btn primary"
    on:click={() => dispatch('edit')}
    title="rhwp로 HWP 문단을 직접 편집"
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
    <span>편집</span>
  </button>

  <button
    type="button"
    class="sb-btn"
    disabled={roundtripping}
    on:click={roundTripCopy}
    title="rhwp의 parse → serialize를 거쳐 `-copy` 접미사로 저장"
  >
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
      <path
        d="M19 14v5a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h5"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linejoin="round"
      />
      <path d="M15 3h6v6" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
      <path d="M10 14L21 3" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
    </svg>
    <span>{roundtripping ? '저장 중…' : 'HWP 복사본'}</span>
  </button>

  {#if feedback}
    <div class="feedback">{feedback}</div>
  {/if}

  <div class="spacer"></div>

  <p class="foot">
    편집·저장은 vendor된 <code>rhwp 0.7.2</code>에서 직접 수행됩니다. 원본 파일은 수정되지 않습니다.
  </p>
</aside>

<style>
  .hwp-sidebar {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    width: 160px;
    min-width: 160px;
    border-right: 1px solid var(--color-separator-non-opaque);
    background: color-mix(in srgb, var(--color-accent) 4%, var(--color-bg-card));
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 4px 6px;
    border-bottom: 1px solid var(--color-separator-non-opaque);
  }

  .badge {
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--color-accent);
    color: white;
    font-size: var(--text-caption2-size);
    font-weight: 700;
    letter-spacing: 0.05em;
  }

  .caption {
    font-size: var(--text-caption2-size);
    color: var(--color-label-tertiary);
  }

  .sb-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
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
    opacity: 0.45;
    cursor: not-allowed;
  }

  .sb-btn.primary {
    background: var(--color-accent);
    color: white;
    border-color: var(--color-accent);
  }

  .sb-btn.primary svg {
    color: white;
  }

  .sb-btn.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-accent) 88%, black);
  }

  .feedback {
    padding: 6px 8px;
    border-radius: var(--radius-xs);
    background: color-mix(in srgb, var(--color-success) 15%, transparent);
    color: var(--color-success);
    font-size: var(--text-caption1-size);
    line-height: 1.3;
  }

  .spacer {
    flex: 1;
  }

  .foot {
    margin: 0;
    padding: var(--space-2) 4px 0;
    border-top: 1px solid var(--color-separator-non-opaque);
    font-size: var(--text-caption2-size);
    color: var(--color-label-tertiary);
    line-height: 1.4;
  }

  .foot code {
    padding: 1px 4px;
    border-radius: var(--radius-xs);
    background: var(--color-fill-quaternary);
    font-size: 0.92em;
  }
</style>
