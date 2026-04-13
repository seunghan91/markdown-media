<script lang="ts">
  export let progress = 0;
  export let label = '진행률';
</script>

<div
  class="progress-shell"
  role="progressbar"
  aria-label={label}
  aria-valuemin="0"
  aria-valuemax="100"
  aria-valuenow={Math.round(progress)}
>
  <div class="progress-meta">
    <span>{label}</span>
    <strong>{Math.round(progress)}%</strong>
  </div>

  <div class="track">
    <div class="fill" style={`width: ${Math.max(0, Math.min(100, progress))}%`}></div>
  </div>
</div>

<style>
  .progress-shell {
    display: grid;
    gap: 10px;
  }

  .progress-meta {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    color: var(--color-label-secondary);
    font-size: var(--text-footnote);
  }

  .track {
    overflow: hidden;
    height: 10px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--color-fill-primary) 95%, transparent);
  }

  .fill {
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--color-accent), var(--progress-gradient-end));
  }
</style>
