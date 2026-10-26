<script lang="ts">
  // 360° equirectangular photo viewer (lazy loads @photo-sphere-viewer/core).
  let { src }: { src: string } = $props();

  let container = $state<HTMLDivElement | null>(null);
  let failed = $state(false);

  $effect(() => {
    const el = container;
    const url = src;
    if (!el) return;
    let viewer: { destroy: () => void } | null = null;
    let cancelled = false;
    (async () => {
      try {
        const [{ Viewer }] = await Promise.all([
          import('@photo-sphere-viewer/core'),
          import('@photo-sphere-viewer/core/index.css'),
        ]);
        if (cancelled) return;
        viewer = new Viewer({
          container: el,
          panorama: url,
          navbar: ['zoom', 'move', 'fullscreen'],
          defaultZoomLvl: 0,
          touchmoveTwoFingers: false,
          mousewheelCtrlKey: false,
        });
      } catch {
        failed = true;
      }
    })();
    return () => {
      cancelled = true;
      viewer?.destroy();
    };
  });
</script>

{#if failed}
  <img class="fallback" {src} alt="" />
{:else}
  <div class="sphere" bind:this={container}></div>
{/if}

<style>
  .sphere {
    width: 100%;
    height: 100%;
  }
  .fallback {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
</style>
