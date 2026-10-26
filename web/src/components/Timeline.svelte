<script lang="ts">
  import AssetGrid from './AssetGrid.svelte';
  import { api, ApiError, type AssetDto, type TimelineFilters } from '../lib/api';
  import type { Selection } from '../lib/selection.svelte';
  import { toastError, ui, type ViewerContext } from '../lib/state.svelte';
  import { monthLabel } from '../lib/utils';
  import { t } from '../lib/i18n';
  import type { Snippet } from 'svelte';

  let {
    filters = {},
    selection,
    emptyText = t('timelineEmpty'),
    pickMode = false,
    viewerOpts = {},
    onerror,
    header,
    scrubber = true,
  }: {
    filters?: TimelineFilters;
    selection?: Selection;
    emptyText?: string;
    pickMode?: boolean;
    viewerOpts?: Partial<ViewerContext>;
    onerror?: (e: ApiError) => void;
    header?: Snippet;
    scrubber?: boolean;
  } = $props();

  interface Bucket {
    timeBucket: string;
    count: number;
    assets: AssetDto[] | null;
    loading: boolean;
  }

  let buckets = $state<Bucket[]>([]);
  let loading = $state(true);
  let width = $state(1000);
  const filterKey = $derived(JSON.stringify(filters));
  let generation = 0;

  async function loadBuckets(key: string) {
    const gen = ++generation;
    loading = true;
    try {
      const res = await api.buckets(JSON.parse(key));
      if (gen !== generation) return;
      buckets = res.map((b) => ({ ...b, assets: null, loading: false }));
      // Load the first months right away; the rest lazily via IntersectionObserver.
      buckets.slice(0, 2).forEach((b) => loadBucket(b));
    } catch (e) {
      if (gen !== generation) return;
      buckets = [];
      if (e instanceof ApiError && onerror) onerror(e);
      else toastError(e);
    } finally {
      if (gen === generation) loading = false;
    }
  }

  $effect(() => {
    void ui.assetsVersion;
    loadBuckets(filterKey);
  });

  async function loadBucket(b: Bucket) {
    if (b.assets || b.loading) return;
    b.loading = true;
    const gen = generation;
    try {
      const assets = await api.bucket(b.timeBucket, JSON.parse(filterKey));
      if (gen !== generation) return;
      b.assets = assets;
    } catch (e) {
      if (e instanceof ApiError && onerror) onerror(e);
    } finally {
      b.loading = false;
    }
  }

  export function reload() {
    loadBuckets(filterKey);
  }

  function observe(node: HTMLElement, b: Bucket) {
    let bucket = b;
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) loadBucket(bucket);
      },
      { rootMargin: '800px 0px' },
    );
    io.observe(node);
    return {
      update(nb: Bucket) {
        bucket = nb;
      },
      destroy() {
        io.disconnect();
      },
    };
  }

  function estimateHeight(count: number) {
    const rowH = width < 600 ? 110 : width < 1000 ? 170 : 220;
    const perRow = Math.max(1, Math.floor(width / (rowH * 1.25)));
    return Math.ceil(count / perRow) * (rowH + 4) + 48 * Math.min(6, Math.ceil(count / perRow));
  }

  function allLoaded(): AssetDto[] {
    const out: AssetDto[] = [];
    for (const b of buckets) if (b.assets) out.push(...b.assets);
    return out;
  }

  // Scrubber: year markers positioned by cumulative count.
  const years = $derived.by(() => {
    const total = buckets.reduce((s, b) => s + b.count, 0) || 1;
    const out: { year: string; pos: number; bucket: string }[] = [];
    let acc = 0;
    for (const b of buckets) {
      const y = b.timeBucket.slice(0, 4);
      if (!out.length || out[out.length - 1].year !== y) out.push({ year: y, pos: acc / total, bucket: b.timeBucket });
      acc += b.count;
    }
    // drop labels that would overlap
    const filtered: typeof out = [];
    for (const y of out) if (!filtered.length || y.pos - filtered[filtered.length - 1].pos > 0.04) filtered.push(y);
    return filtered;
  });

  function scrollToBucket(tb: string) {
    document.getElementById(`bucket-${tb}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

</script>

<div class="timeline" bind:clientWidth={width}>
  {@render header?.()}
  {#if loading && !buckets.length}
    <div class="spinner"></div>
  {:else if !buckets.length}
    <div class="empty">{emptyText}</div>
  {:else}
    {#each buckets as b (b.timeBucket)}
      <section class="bucket" id="bucket-{b.timeBucket}" use:observe={b}>
        <h2 class="month">{monthLabel(b.timeBucket)}</h2>
        {#if b.assets}
          <AssetGrid
            assets={b.assets}
            {selection}
            {pickMode}
            viewerAssets={allLoaded}
            {viewerOpts}
            onchange={reload}
          />
        {:else}
          <div class="placeholder" style="height: {estimateHeight(b.count)}px"></div>
        {/if}
      </section>
    {/each}
  {/if}
</div>

{#if scrubber && years.length > 1 && !pickMode}
  <nav class="scrubber" aria-label={t('years')}>
    {#each years as y (y.year)}
      <button style="top: {y.pos * 100}%" onclick={() => scrollToBucket(y.bucket)}>{y.year}</button>
    {/each}
  </nav>
{/if}

<style>
  .timeline {
    padding-right: 36px;
  }
  .month {
    font-size: 1.15rem;
    padding: 20px 0 0;
  }
  .placeholder {
    background: repeating-linear-gradient(
      90deg,
      var(--placeholder) 0 calc(20% - 4px),
      transparent calc(20% - 4px) 20%
    );
    opacity: 0.5;
    border-radius: 4px;
    margin-top: 12px;
  }
  .scrubber {
    position: fixed;
    right: 0;
    top: calc(var(--topbar-h) + 24px);
    bottom: 24px;
    width: 40px;
    z-index: 5;
  }
  .scrubber button {
    position: absolute;
    right: 4px;
    transform: translateY(-50%);
    border: none;
    background: var(--bg);
    color: var(--fg-muted);
    font-size: 0.72rem;
    padding: 2px 4px;
    border-radius: 6px;
    cursor: pointer;
  }
  .scrubber button:hover {
    color: var(--primary);
    background: var(--bg-soft);
  }
  @media (max-width: 700px) {
    .timeline {
      padding-right: 0;
    }
    .scrubber {
      display: none;
    }
  }
</style>
