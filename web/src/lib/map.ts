// Lazy loading of maplibre-gl with an OpenStreetMap raster style.
import type { StyleSpecification } from 'maplibre-gl';
import { auth } from './state.svelte';

const OSM_TILES = 'https://tile.openstreetmap.org/{z}/{x}/{y}.png';
const OSM_ATTRIBUTION = '© <a href="https://www.openstreetmap.org/copyright" target="_blank">OpenStreetMap</a>';

type Maplibre = typeof import('maplibre-gl');
let loader: Promise<Maplibre> | null = null;

export function loadMaplibre(): Promise<Maplibre> {
  if (!loader) {
    loader = Promise.all([import('maplibre-gl'), import('maplibre-gl/dist/maplibre-gl.css')]).then(([m]) => {
      const mod = m as Maplibre & { default?: Maplibre };
      return (mod.default ?? mod) as Maplibre;
    });
  }
  return loader;
}

export function osmStyle(): StyleSpecification {
  return {
    version: 8,
    sources: {
      osm: {
        type: 'raster',
        tiles: [auth.serverInfo?.mapTileUrl || OSM_TILES],
        tileSize: 256,
        maxzoom: 19,
        attribution: auth.serverInfo?.mapTileUrl ? (auth.serverInfo.mapAttribution ?? '') : OSM_ATTRIBUTION,
      },
    },
    layers: [{ id: 'osm', type: 'raster', source: 'osm' }],
  };
}
