<script lang="ts">
  import AssetGrid from './AssetGrid.svelte';
  import EmptyState from './EmptyState.svelte';
  import type { IconName } from './Icon.svelte';
  import { api, ApiError, type AssetDto, type TimelineFilters } from '../lib/api';
  import type { Selection } from '../lib/selection.svelte';
  import { toastError, ui, type ViewerContext } from '../lib/state.svelte';
  import { monthLabel } from '../lib/utils';
  import { t } from '../lib/i18n';
  import type { Snippet } from 'svelte';

  let {
    filters = {},
    selection,
    emptyText = t('timelineEmptyTitle'),
    emptyHint = t('timelineEmptyHint'),
    emptyIcon = 'photo',
    pickMode = false,
    viewerOpts = {},
    onerror,
    header,
    scrubber = true,
  }: {
    filters?: TimelineFilters;
    selection?: Selection;
    emptyText?: string;
    emptyHint?: string;
    emptyIcon?: IconName;
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

  // Scrubber: maps a fraction of the bar to a bucket (by cumulative count) and back.
  const spans = $derived.by(() => {
    const total = buckets.reduce((s, b) => s + b.count, 0) || 1;
    let acc = 0;
    return buckets.map((b) => {
      const start = acc / total;
      acc += b.count;
      return { bucket: b.timeBucket, start, end: acc / total };
    });
  });
  let bar = $state<HTMLElement>();
  let hoverFrac = $state<number | null>(null);
  let scrollFrac = $state(0);
  let dragging = $state(false);

  function spanAt(f: number) {
    return spans.find((x) => f < x.end) ?? spans[spans.length - 1];
  }
  function fracOf(e: PointerEvent) {
    const r = bar!.getBoundingClientRect();
    return Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
  }
  function jumpTo(f: number) {
    const sp = spanAt(f);
    const el = sp && document.getElementById(`bucket-${sp.bucket}`);
    if (!el) return;
    const within = sp.end > sp.start ? (f - sp.start) / (sp.end - sp.start) : 0;
    const top = el.getBoundingClientRect().top + window.scrollY - 72 + within * el.offsetHeight;
    window.scrollTo({ top });
  }
  function onScroll() {
    const max = document.documentElement.scrollHeight - window.innerHeight;
    scrollFrac = max > 0 ? window.scrollY / max : 0;
  }
  const hoverLabel = $derived.by(() => {
    const f = hoverFrac ?? (dragging ? scrollFrac : null);
    if (f === null || !spans.length) return '';
    return monthLabel(spanAt(f).bucket);
  });

</script>

<svelte:window onscroll={onScroll} />

<div class="timeline" class:with-scrubber={scrubber && !pickMode} bind:clientWidth={width}>
  {@render header?.()}
  {#if loading && !buckets.length}
    <div class="spinner"></div>
  {:else if !buckets.length}
    <EmptyState icon={emptyIcon} title={emptyText} text={emptyHint} />
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
  <nav
    class="scrubber"
    class:active={hoverFrac !== null || dragging}
    aria-label={t('years')}
    bind:this={bar}
    onpointermove={(e) => {
      hoverFrac = fracOf(e);
      if (dragging) jumpTo(hoverFrac);
    }}
    onpointerleave={() => (hoverFrac = null)}
    onpointerdown={(e) => {
      dragging = true;
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      jumpTo(fracOf(e));
    }}
    onpointerup={() => (dragging = false)}
    onpointercancel={() => (dragging = false)}
  >
    {#each years as y (y.year)}
      <button
        style="top: {y.pos * 100}%"
        tabindex="-1"
        onclick={(e) => {
          e.stopPropagation();
          scrollToBucket(y.bucket);
        }}>{y.year}</button
      >
    {/each}
    <span class="pos" style="top: {scrollFrac * 100}%"></span>
    {#if hoverFrac !== null}
      <span class="hover" style="top: {hoverFrac * 100}%"><span class="bubble">{hoverLabel}</span></span>
    {/if}
  </nav>
{/if}

<style>
  .timeline.with-scrubber {
    padding-right: 44px;
  }
  .month {
    font-size: 1.3rem;
    font-weight: 600;
    padding: 28px 0 0;
    letter-spacing: -0.01em;
  }
  .bucket:first-of-type .month {
    padding-top: 12px;
  }
  .placeholder {
    background: repeating-linear-gradient(
      90deg,
      var(--placeholder) 0 calc(20% - 4px),
      transparent calc(20% - 4px) 20%
    );
    opacity: 0.6;
    margin-top: 44px;
  }
  .scrubber {
    position: fixed;
    right: 0;
    top: calc(var(--topbar-h) + 16px);
    bottom: 16px;
    width: 56px;
    z-index: 30;
    cursor: row-resize;
    user-select: none;
    touch-action: none;
  }
  .scrubber button {
    position: absolute;
    right: 8px;
    transform: translateY(-50%);
    border: none;
    background: transparent;
    color: var(--fg-muted);
    font-size: 0.7rem;
    font-weight: 600;
    padding: 1px 4px;
    border-radius: 6px;
    cursor: pointer;
    transition: color 0.15s;
  }
  .scrubber button:hover {
    color: var(--primary);
  }
  .pos {
    position: absolute;
    right: 0;
    width: 4px;
    height: 28px;
    margin-top: -14px;
    border-radius: 2px 0 0 2px;
    background: var(--primary);
    opacity: 0.55;
    pointer-events: none;
    transition: opacity 0.15s;
  }
  .scrubber.active .pos {
    opacity: 1;
  }
  .hover {
    position: absolute;
    right: 0;
    width: 40px;
    height: 2px;
    background: var(--primary);
    pointer-events: none;
  }
  .bubble {
    position: absolute;
    right: 48px;
    top: 50%;
    transform: translateY(-50%);
    white-space: nowrap;
    padding: 4px 10px;
    border-radius: 8px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--fg);
  }
  @media (max-width: 700px) {
    .timeline.with-scrubber {
      padding-right: 0;
    }
    .scrubber {
      display: none;
    }
  }
</style>
