<script lang="ts">
  import AssetGrid from '../components/AssetGrid.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import SearchFilterDialog from '../components/SearchFilterDialog.svelte';
  import Icon from '../components/Icon.svelte';
  import { api, type AssetDto, type MetadataSearch } from '../lib/api';
  import { Selection } from '../lib/selection.svelte';
  import { auth, toastError, ui } from '../lib/state.svelte';
  import { router, setQuery } from '../lib/router.svelte';
  import { t } from '../lib/i18n';

  const selection = new Selection();
  let assets = $state<AssetDto[]>([]);
  let total = $state(0);
  let nextPage = $state<number | null>(null);
  let loading = $state(false);
  let searched = $state(false);
  let dialog = $state(false);
  let sentinel = $state<HTMLDivElement | null>(null);

  const q = $derived(router.query.get('q') ?? '');
  const filter = $derived.by<MetadataSearch>(() => {
    const raw = router.query.get('f');
    if (!raw) return {};
    try {
      return JSON.parse(raw) as MetadataSearch;
    } catch {
      return {};
    }
  });
  const key = $derived(JSON.stringify([q, filter]));
  let gen = 0;

  $effect(() => {
    if (router.query.get('filter') === '1') {
      dialog = true;
      setQuery({ filter: null });
    }
  });

  $effect(() => {
    void ui.assetsVersion;
    run(key, 1);
  });

  async function run(k: string, page: number) {
    const [query, f] = JSON.parse(k) as [string, MetadataSearch];
    if (!query && !Object.keys(f).length) {
      assets = [];
      searched = false;
      return;
    }
    const g = page === 1 ? ++gen : gen;
    loading = true;
    try {
      let res;
      if (query) {
        try {
          if (auth.serverInfo && !auth.serverInfo.features.smartSearch) throw new Error('no smart search');
          res = await api.smartSearch({ ...f, query, page, size: 100 });
        } catch (e) {
          if (page === 1 && !(e instanceof Error && e.message === 'no smart search')) console.warn(e);
          res = await api.metadataSearch({ ...f, originalFileName: query, page, size: 100 });
        }
      } else {
        res = await api.metadataSearch({ ...f, page, size: 100 });
      }
      if (g !== gen) return;
      assets = page === 1 ? res.assets : [...assets, ...res.assets];
      total = res.total;
      nextPage = res.nextPage;
      searched = true;
    } catch (e) {
      toastError(e);
    } finally {
      if (g === gen) loading = false;
    }
  }

  $effect(() => {
    const el = sentinel;
    if (!el) return;
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting) && nextPage && !loading) run(key, nextPage);
    });
    io.observe(el);
    return () => io.disconnect();
  });

  function applyFilter(f: MetadataSearch) {
    dialog = false;
    setQuery({ f: Object.keys(f).length ? JSON.stringify(f) : null }, false);
  }

  const chips = $derived(
    Object.entries(filter).map(([k, v]) => ({ k, label: `${k}: ${Array.isArray(v) ? v.length : String(v).slice(0, 24)}` })),
  );

  function removeChip(k: string) {
    const f = { ...filter } as Record<string, unknown>;
    delete f[k];
    setQuery({ f: Object.keys(f).length ? JSON.stringify(f) : null });
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{q ? `„${q}“` : t('search')}</h1>
    <button class="btn" onclick={() => (dialog = true)}><Icon name="tune" size={20} /> {t('filter')}</button>
  </div>
  {#if chips.length}
    <div class="row wrap chips">
      {#each chips as c (c.k)}
        <span class="chip active">{c.label} <button class="x" onclick={() => removeChip(c.k)} aria-label={t('remove')}><Icon name="close" size={14} /></button></span>
      {/each}
    </div>
  {/if}
  {#if !q && !chips.length}
    <div class="empty">{t('smartSearchHint')}</div>
  {:else if loading && !assets.length}
    <div class="spinner"></div>
  {:else if searched && !assets.length}
    <div class="empty">{t('noResults')}</div>
  {:else}
    <p class="muted small">{t('resultsCount', { n: total })}</p>
    <AssetGrid {assets} {selection} groupByDay={false} />
    <div bind:this={sentinel} class="sentinel"></div>
    {#if loading}<div class="spinner"></div>{/if}
  {/if}
</div>
<SelectionBar {selection} context="search" />

{#if dialog}
  <SearchFilterDialog initial={filter} onapply={applyFilter} onclose={() => (dialog = false)} />
{/if}

<style>
  .chips {
    margin-bottom: 12px;
  }
  .x {
    border: none;
    background: none;
    padding: 0;
    display: flex;
    cursor: pointer;
    color: inherit;
  }
  .sentinel {
    height: 1px;
  }
</style>
