<script lang="ts">
  import { loadMaplibre, osmStyle } from '../lib/map';

  let { lat, lon, zoom = 12 }: { lat: number; lon: number; zoom?: number } = $props();
  let el = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const node = el;
    const center: [number, number] = [lon, lat];
    if (!node) return;
    let map: { remove: () => void } | null = null;
    let cancelled = false;
    loadMaplibre().then((ml) => {
      if (cancelled) return;
      const m = new ml.Map({ container: node, style: osmStyle(), center, zoom, attributionControl: { compact: true } });
      new ml.Marker({ color: '#1a73e8' }).setLngLat(center).addTo(m);
      map = m;
    });
    return () => {
      cancelled = true;
      map?.remove();
    };
  });
</script>

<div class="mini" bind:this={el}></div>
<a class="small" href="https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=15/{lat}/{lon}" target="_blank" rel="noopener"
  >{lat.toFixed(5)}, {lon.toFixed(5)}</a
>

<style>
  .mini {
    height: 180px;
    border-radius: 12px;
    overflow: hidden;
    margin: 8px 0 4px;
    background: var(--bg-soft);
  }
</style>
