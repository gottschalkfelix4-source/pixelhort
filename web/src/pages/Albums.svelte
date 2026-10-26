<script lang="ts">
  import Icon from '../components/Icon.svelte';
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
    <button class="btn primary" onclick={create}><Icon name="add" size={20} /> {t('newAlbum')}</button>
  </div>
  <div class="tabs">
    <button class:active={tab === 'all'} onclick={() => setQuery({ tab: null })}>{t('all')}</button>
    <button class:active={tab === 'mine'} onclick={() => setQuery({ tab: 'mine' })}>{t('myAlbums')}</button>
    <button class:active={tab === 'shared'} onclick={() => setQuery({ tab: 'shared' })}>{t('sharedAlbums')}</button>
  </div>
  {#if loading && !albums.length}
    <div class="spinner"></div>
  {:else if !albums.length}
    <div class="empty">{t('noAlbums')}</div>
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
          </div>
          <div class="title">{a.albumName || t('untitledAlbum')}</div>
          <div class="muted small">
            {t('photosCount', { n: a.assetCount })}{range(a) ? ` · ${range(a)}` : ''}
            {#if a.shared}· <Icon name="people" size={14} />{/if}
            {#if a.ownerId !== auth.user?.id}· {a.owner.name}{/if}
          </div>
        </a>
      {/each}
    </div>
  {/if}
</div>

<style>
  .no-cover {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-muted);
  }
</style>
