<script lang="ts">
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import { onMount } from 'svelte';

  onMount(() => {
    if (browser) {
      goto('/convert', { replaceState: true });
    }
  });
</script>

<div style="display:flex;align-items:center;justify-content:center;min-height:60vh;">
  <p style="color:var(--color-label-tertiary);">로딩 중...</p>
</div>
