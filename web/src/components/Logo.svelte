<script lang="ts" module>
  let seq = 0;
</script>

<script lang="ts">
  // The Pixelhort mark (docs/brand/pixelhort-logo.svg): a roof sheltering a 2×2 pixel grid.
  let { size = 30, wordmark = false }: { size?: number; wordmark?: boolean } = $props();
  // Gradient ids must be unique per document, otherwise a hidden instance breaks the others.
  const gid = `ph-bg-${++seq}`;
</script>

<svg viewBox="0 0 64 64" width={size} height={size} aria-hidden="true" class="mark">
  <defs>
    <linearGradient id={gid} x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#3b8cff" />
      <stop offset="1" stop-color="#1557c9" />
    </linearGradient>
  </defs>
  <rect width="64" height="64" rx="15" fill="url(#{gid})" />
  <path d="M15 28 32 13.5 49 28" fill="none" stroke="#fff" stroke-width="5.5" stroke-linecap="round" stroke-linejoin="round" />
  <rect x="20.5" y="28.5" width="10.5" height="10.5" rx="2.5" fill="#fff" />
  <rect x="33" y="28.5" width="10.5" height="10.5" rx="2.5" fill="#ffc42e" />
  <rect x="20.5" y="41" width="10.5" height="10.5" rx="2.5" fill="#fff" />
  <rect x="33" y="41" width="10.5" height="10.5" rx="2.5" fill="#fff" fill-opacity=".55" />
</svg>
{#if wordmark}<span class="wordmark">Pixel<b>hort</b></span>{/if}

<style>
  .mark {
    flex-shrink: 0;
  }
  .wordmark {
    font-weight: 500;
    letter-spacing: -0.01em;
    color: var(--fg);
  }
  .wordmark b {
    font-weight: 700;
    color: var(--primary);
  }
</style>
