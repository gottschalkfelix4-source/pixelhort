<script lang="ts">
  import { api, thumbnailUrl, type AssetDto, type MapMarker } from '../lib/api';
  import { loadMaplibre, osmStyle } from '../lib/map';
  import { openViewer, toastError, ui } from '../lib/state.svelte';
  import { t } from '../lib/i18n';
  import type { GeoJSONSource, Map as MlMap, Marker } from 'maplibre-gl';

  let el = $state<HTMLDivElement | null>(null);
  let markers = $state<MapMarker[]>([]);
  let onlyFav = $state(false);
  let loaded = $state(false);
  let map: MlMap | null = null;
  let html = new Map<string, Marker>();

  async function loadMarkers(fav: boolean, partners: boolean) {
    try {
      markers = await api.mapMarkers({ isFavorite: fav || undefined, withPartners: partners || undefined });
    } catch (e) {
      toastError(e);
      markers = [];
    } finally {
      loaded = true;
    }
  }

  $effect(() => {
    loadMarkers(onlyFav, ui.withPartners);
  });

  // Create the map once.
  $effect(() => {
    const node = el;
    if (!node) return;
    let cancelled = false;
    let observer: ResizeObserver | null = null;
    loadMaplibre().then((ml) => {
      if (cancelled) return;
      const m = new ml.Map({ container: node, style: osmStyle(), center: [10, 50], zoom: 3, attributionControl: { compact: true } });
      // The container gets its final size after layout; keep the canvas in sync.
      observer = new ResizeObserver(() => m.resize());
      observer.observe(node);
      m.addControl(new ml.NavigationControl({ showCompass: false }), 'top-right');
      // 'load' waits for all tiles; the style (inline) is ready much earlier.
      const init = () => {
        if (cancelled || m.getSource('assets') || !m.isStyleLoaded()) return;
        m.addSource('assets', {
          type: 'geojson',
          data: { type: 'FeatureCollection', features: [] },
          cluster: true,
          clusterRadius: 60,
          clusterMaxZoom: 17,
          clusterProperties: { minIdx: ['min', ['get', 'idx']] },
        });
        // invisible layer so features get rendered & queryable
        m.addLayer({ id: 'assets-hit', type: 'circle', source: 'assets', paint: { 'circle-radius': 1, 'circle-opacity': 0 } });
        map = m;
        pushData();
        fit();
      };
      m.on('styledata', init);
      m.on('load', init);
      init();
      m.on('render', () => {
        if (map) updateHtmlMarkers(ml);
      });
    });
    return () => {
      cancelled = true;
      observer?.disconnect();
      html.forEach((mk) => mk.remove());
      html = new Map();
      map?.remove();
      map = null;
    };
  });

  $effect(() => {
    void markers;
    pushData();
  });

  function pushData() {
    const src = map?.getSource('assets') as GeoJSONSource | undefined;
    if (!src) return;
    html.forEach((mk) => mk.remove());
    html.clear();
    src.setData({
      type: 'FeatureCollection',
      features: markers.map((mk, idx) => ({
        type: 'Feature',
        geometry: { type: 'Point', coordinates: [mk.lon, mk.lat] },
        properties: { id: mk.id, idx },
      })),
    });
  }

  let fitted = false;
  function fit() {
    if (!map || fitted || !markers.length) return;
    fitted = true;
    let [minX, minY, maxX, maxY] = [180, 90, -180, -90];
    for (const mk of markers) {
      minX = Math.min(minX, mk.lon);
      maxX = Math.max(maxX, mk.lon);
      minY = Math.min(minY, mk.lat);
      maxY = Math.max(maxY, mk.lat);
    }
    map.fitBounds(
      [
        [minX, minY],
        [maxX, maxY],
      ],
      { padding: 60, maxZoom: 14, duration: 0 },
    );
  }
  $effect(() => {
    if (markers.length && map) fit();
  });

  function updateHtmlMarkers(ml: typeof import('maplibre-gl')) {
    if (!map) return;
    const feats = map.querySourceFeatures('assets');
    const seen = new Set<string>();
    for (const f of feats) {
      const p = f.properties as { cluster?: boolean; cluster_id?: number; point_count?: number; minIdx?: number; idx?: number; id?: string };
      const key = p.cluster ? `c${p.cluster_id}` : `p${p.id}`;
      if (seen.has(key)) continue;
      seen.add(key);
      if (html.has(key)) continue;
      const idx = p.cluster ? (p.minIdx ?? 0) : (p.idx ?? 0);
      const mk = markers[idx];
      if (!mk) continue;
      const div = document.createElement('div');
      div.className = 'ph-marker';
      div.style.backgroundImage = `url(${thumbnailUrl(mk.id)})`;
      if (p.cluster) {
        const c = document.createElement('span');
        c.textContent = String(p.point_count ?? '');
        div.appendChild(c);
      }
      div.addEventListener('click', (e) => {
        e.stopPropagation();
        if (p.cluster) clickCluster(p.cluster_id!, (f.geometry as unknown as { coordinates: [number, number] }).coordinates);
        else openAssets([mk.id]);
      });
      const marker = new ml.Marker({ element: div }).setLngLat((f.geometry as unknown as { coordinates: [number, number] }).coordinates).addTo(map);
      html.set(key, marker);
    }
    for (const [k, m] of html) {
      if (!seen.has(k)) {
        m.remove();
        html.delete(k);
      }
    }
  }

  async function clickCluster(clusterId: number, center: [number, number]) {
    if (!map) return;
    const src = map.getSource('assets') as GeoJSONSource;
    try {
      const zoom = await src.getClusterExpansionZoom(clusterId);
      if (zoom <= 17 && map.getZoom() < 17) {
        map.easeTo({ center, zoom: Math.min(zoom, 18) });
        return;
      }
      const leaves = await src.getClusterLeaves(clusterId, 200, 0);
      openAssets(leaves.map((l) => (l.properties as { id: string }).id));
    } catch (e) {
      toastError(e);
    }
  }

  async function openAssets(ids: string[]) {
    try {
      const assets = (await Promise.all(ids.slice(0, 100).map((id) => api.asset(id).catch(() => null)))).filter(
        (a): a is AssetDto => !!a,
      );
      if (assets.length) openViewer({ assets, index: 0 });
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="map-page">
  <div class="map" bind:this={el}></div>
  <div class="controls">
    <label class="chip" class:active={onlyFav}><input type="checkbox" bind:checked={onlyFav} hidden /> {t('onlyFavoritesShort')}</label>
  </div>
  {#if loaded && !markers.length}
    <div class="no-markers">{t('mapNoMarkers')}</div>
  {/if}
</div>

<style>
  .map-page {
    position: relative;
    height: calc(100vh - var(--topbar-h));
  }
  .map {
    position: absolute;
    inset: 0;
  }
  .controls {
    position: absolute;
    top: 12px;
    left: 12px;
    z-index: 2;
    display: flex;
    gap: 8px;
  }
  .controls .chip {
    box-shadow: var(--shadow);
  }
  .no-markers {
    position: absolute;
    top: 12px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-elev);
    padding: 8px 16px;
    border-radius: 16px;
    box-shadow: var(--shadow);
    z-index: 2;
  }
  :global(.ph-marker) {
    width: 52px;
    height: 52px;
    border-radius: 10px;
    border: 3px solid #fff;
    background: #ccc center/cover no-repeat;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
    cursor: pointer;
    position: relative;
  }
  :global(.ph-marker span) {
    position: absolute;
    top: -10px;
    right: -10px;
    background: #1a73e8;
    color: #fff;
    font-size: 11px;
    font-weight: 600;
    border-radius: 10px;
    padding: 1px 6px;
    min-width: 20px;
    text-align: center;
    font-family: sans-serif;
  }
</style>
