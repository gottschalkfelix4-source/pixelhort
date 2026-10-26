import { SvelteMap } from 'svelte/reactivity';
import type { AssetDto } from './api';

/** Multi-selection of assets. Keeps the asset objects for bulk actions. */
export class Selection {
  map = new SvelteMap<string, AssetDto>();
  /** id of the last toggled asset (for shift range selection) */
  anchor: string | null = null;

  get size() {
    return this.map.size;
  }
  get active() {
    return this.map.size > 0;
  }
  has(id: string) {
    return this.map.has(id);
  }
  toggle(a: AssetDto) {
    if (this.map.has(a.id)) this.map.delete(a.id);
    else this.map.set(a.id, a);
    this.anchor = a.id;
  }
  add(list: AssetDto[]) {
    for (const a of list) this.map.set(a.id, a);
  }
  removeAll(list: AssetDto[]) {
    for (const a of list) this.map.delete(a.id);
  }
  clear() {
    this.map.clear();
    this.anchor = null;
  }
  list(): AssetDto[] {
    return [...this.map.values()];
  }
  ids(): string[] {
    return [...this.map.keys()];
  }
}
