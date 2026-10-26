<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import { api, thumbnailUrl, type AlbumDto } from '../lib/api';
  import { auth, promptDialog, toastError } from '../lib/state.svelte';
  import { navigate, router, setQuery } from '../lib/router.svelte';
  import { formatLocal } from '../lib/utils';
  import { t } from '../lib/i18n';

  let albums = $state<AlbumDto[]>([]);
  let loading = $state(true);
  const tab = $derived((router.query.get('tab') as 'all' | 'mine' | 'shared') || 'all');

  $effect(() => {
    load(tab);
  });

  async function load(which: string) {
    loading = true;
    try {
      albums = await api.albums(which === 'shared' ? true : which === 'mine' ? false : undefined);
    } catch (e) {
      toastError(e);
    } finally {
      loading = false;
    }
  }

  async function create() {
    const name = await promptDialog(t('newAlbum'), { placeholder: t('albumName'), confirmLabel: t('create') });
    if (name === null) return;
    try {
      const a = await api.createAlbum({ albumName: name.trim() || t('untitledAlbum') });
      navigate(`/albums/${a.id}`);
    } catch (e) {
      toastError(e);
    }
  }

  function range(a: AlbumDto) {
    if (!a.startDate) return '';
    const s = formatLocal(a.startDate, { month: 'short', year: 'numeric' });
    const e = a.endDate ? formatLocal(a.endDate, { month: 'short', year: 'numeric' }) : s;
    return s === e ? s : `${s} – ${e}`;
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{t('albums')}</h1>
    <div class="segmented">
      <button class:active={tab === 'all'} onclick={() => setQuery({ tab: null })}>{t('all')}</button>
      <button class:active={tab === 'mine'} onclick={() => setQuery({ tab: 'mine' })}>{t('myAlbums')}</button>
      <button class:active={tab === 'shared'} onclick={() => setQuery({ tab: 'shared' })}>{t('sharedAlbums')}</button>
    </div>
    <button class="btn primary" onclick={create}><Icon name="add" size={20} /> {t('newAlbum')}</button>
  </div>
  {#if loading && !albums.length}
    <div class="spinner"></div>
  {:else if !albums.length}
    <EmptyState icon="album" title={t('noAlbums')} text={t('noAlbumsHint')}>
      <button class="btn primary" onclick={create}><Icon name="add" size={20} /> {t('newAlbum')}</button>
    </EmptyState>
  {:else}
    <div class="cover-grid">
      {#each albums as a (a.id)}
        <a class="cover-card" href="/albums/{a.id}">
          <div class="cover">
            {#if a.albumThumbnailAssetId}
              <img src={thumbnailUrl(a.albumThumbnailAssetId)} alt="" loading="lazy" />
            {:else}
              <div class="no-cover"><Icon name="album" size={48} /></div>
            {/if}
            {#if a.shared}<span class="shared-pill" title={t('sharedAlbums')}><Icon name="people" size={16} /></span>{/if}
          </div>
          <div class="title">{a.albumName || t('untitledAlbum')}</div>
          <div class="muted small meta">
            {t('photosCount', { n: a.assetCount })}{range(a) ? ` · ${range(a)}` : ''}
          </div>
          {#if a.ownerId !== auth.user?.id}<div class="muted small meta owner"><Icon name="person" size={14} /> {a.owner.name}</div>{/if}
        </a>
      {/each}
    </div>
  {/if}
</div>

<style>
  .cover {
    position: relative;
  }
  .shared-pill {
    position: absolute;
    right: 8px;
    bottom: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    color: #fff;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(6px);
  }
  .meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .owner {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .no-cover {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-muted);
    background: var(--bg-soft);
  }
  @media (max-width: 700px) {
    .segmented {
      order: 3;
      width: 100%;
    }
  }
</style>
