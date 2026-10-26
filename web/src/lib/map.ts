// Lazy loading of maplibre-gl with an OpenStreetMap raster style.
import type { StyleSpecification } from 'maplibre-gl';

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
        tiles: ['https://tile.openstreetmap.org/{z}/{x}/{y}.png'],
        tileSize: 256,
        maxzoom: 19,
        attribution: '© <a href="https://www.openstreetmap.org/copyright" target="_blank">OpenStreetMap</a>',
      },
    },
    layers: [{ id: 'osm', type: 'raster', source: 'osm' }],
  };
}
