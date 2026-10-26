<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import Pickers from '../components/Pickers.svelte';
  import ShareLinkDialog from '../components/ShareLinkDialog.svelte';
  import { api, thumbnailUrl, type PartnerDto, type SharedLinkDto } from '../lib/api';
  import { bumpAssets, confirmDialog, prefs, toast, toastError } from '../lib/state.svelte';
  import { router, setQuery } from '../lib/router.svelte';
  import { copyText, formatAbs } from '../lib/utils';
  import { t } from '../lib/i18n';

  const tab = $derived(router.query.get('tab') === 'partners' || !prefs.value.sharedLinks.enabled ? 'partners' : 'links');
  let links = $state<SharedLinkDto[]>([]);
  let sharedBy = $state<PartnerDto[]>([]);
  let sharedWith = $state<PartnerDto[]>([]);
  let loading = $state(true);
  let editing = $state<SharedLinkDto | null>(null);
  let addingPartner = $state(false);

  async function load() {
    loading = true;
    try {
      [links, sharedBy, sharedWith] = await Promise.all([api.sharedLinks(), api.partners('shared-by'), api.partners('shared-with')]);
    } catch (e) {
      toastError(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    load();
  });

  function cover(l: SharedLinkDto): string | null {
    if (l.album?.albumThumbnailAssetId) return l.album.albumThumbnailAssetId;
    return l.assetIds[0] ?? null;
  }
  function expired(l: SharedLinkDto) {
    return !!l.expiresAt && new Date(l.expiresAt).getTime() < Date.now();
  }

  async function copy(l: SharedLinkDto) {
    await copyText(l.url);
    toast(t('copied'));
  }

  async function remove(l: SharedLinkDto) {
    if (!(await confirmDialog(t('deleteLinkConfirm'), { danger: true, confirmLabel: t('delete') }))) return;
    try {
      await api.deleteSharedLink(l.id);
      links = links.filter((x) => x.id !== l.id);
    } catch (e) {
      toastError(e);
    }
  }

  async function addPartner(userId: string) {
    try {
      const p = await api.addPartner(userId);
      sharedBy = [...sharedBy, p];
    } catch (e) {
      toastError(e);
    }
  }
  async function removePartner(p: PartnerDto) {
    if (!(await confirmDialog(t('removePartnerConfirm', { name: p.user.name }), { danger: true }))) return;
    try {
      await api.removePartner(p.user.id);
      sharedBy = sharedBy.filter((x) => x.user.id !== p.user.id);
    } catch (e) {
      toastError(e);
    }
  }
  async function toggleTimeline(p: PartnerDto) {
    try {
      const r = await api.updatePartner(p.user.id, !p.inTimeline);
      p.inTimeline = r.inTimeline;
      bumpAssets();
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{t('navSharing')}</h1>
    <div class="segmented">
      {#if prefs.value.sharedLinks.enabled}
        <button class:active={tab === 'links'} onclick={() => setQuery({ tab: null })}><Icon name="link" size={18} /> {t('sharedLinks')}</button>
      {/if}
      <button class:active={tab === 'partners'} onclick={() => setQuery({ tab: 'partners' })}><Icon name="people" size={18} /> {t('partners')}</button>
    </div>
  </div>

  {#if loading}
    <div class="spinner"></div>
  {:else if tab === 'links'}
    {#if !links.length}
      <EmptyState icon="link" title={t('noSharedLinks')} text={t('noSharedLinksHint')} />
    {:else}
      <ul class="links">
        {#each links as l (l.id)}
          {@const c = cover(l)}
          <li class="card">
            <div class="thumb">{#if c}<img src={thumbnailUrl(c)} alt="" loading="lazy" />{:else}<Icon name="link" />{/if}</div>
            <div class="info">
              <div class="title">
                {l.description || (l.type === 'ALBUM' ? (l.album?.albumName ?? t('linkAlbum')) : t('linkIndividual', { n: l.assetIds.length }))}
              </div>
              <div class="muted small">
                {l.type === 'ALBUM' ? t('linkAlbum') : t('linkIndividual', { n: l.assetIds.length })} · {formatAbs(l.createdAt, { dateStyle: 'medium' })}
              </div>
              <div class="row wrap badges">
                {#if expired(l)}<span class="badge danger">{t('expired')}</span>
                {:else if l.expiresAt}<span class="badge">{t('expires')}: {formatAbs(l.expiresAt)}</span>{/if}
                {#if l.hasPassword}<span class="badge primary"><Icon name="lock" size={12} /> {t('passwordProtected')}</span>{/if}
                {#if l.allowDownload}<span class="badge"><Icon name="download" size={12} /> {t('download')}</span>{/if}
                {#if l.allowUpload}<span class="badge"><Icon name="upload" size={12} /> {t('upload')}</span>{/if}
              </div>
            </div>
            <div class="row acts">
              <a class="icon-btn" href={l.url} target="_blank" rel="noopener" data-external title={t('open')} aria-label={t('open')}><Icon name="openNew" size={20} /></a>
              <button class="icon-btn" onclick={() => copy(l)} title={t('copyLink')} aria-label={t('copyLink')}><Icon name="copy" size={20} /></button>
              <button class="icon-btn" onclick={() => (editing = l)} title={t('edit')} aria-label={t('edit')}><Icon name="edit" size={20} /></button>
              <button class="icon-btn" onclick={() => remove(l)} title={t('delete')} aria-label={t('delete')}><Icon name="delete" size={20} /></button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <section class="card">
      <div class="section-title">
        <h2>{t('partnerSharedBy')}</h2>
        <span class="spacer"></span>
        <button class="btn small" onclick={() => (addingPartner = true)}><Icon name="add" size={18} /> {t('addPartner')}</button>
      </div>
      {#if !sharedBy.length}<p class="muted small none">{t('noPartnersHint')}</p>{/if}
      <ul class="list">
        {#each sharedBy as p (p.user.id)}
          <li>
            <span class="avatar-initial">{(p.user.name || p.user.email).slice(0, 1).toUpperCase()}</span>
            <div class="grow"><div class="strong">{p.user.name}</div><small class="muted">{p.user.email}</small></div>
            <button class="icon-btn" onclick={() => removePartner(p)} title={t('remove')} aria-label={t('remove')}><Icon name="close" size={20} /></button>
          </li>
        {/each}
      </ul>
    </section>
    <section class="card">
      <div class="section-title"><h2>{t('partnerSharedWith')}</h2></div>
      {#if !sharedWith.length}<p class="muted small none">{t('noPartners')}</p>{/if}
      <ul class="list">
        {#each sharedWith as p (p.user.id)}
          <li>
            <span class="avatar-initial">{(p.user.name || p.user.email).slice(0, 1).toUpperCase()}</span>
            <div class="grow"><div class="strong">{p.user.name}</div><small class="muted">{p.user.email}</small></div>
            <label class="check small"
              >{t('showInTimeline')} <input type="checkbox" class="switch" checked={p.inTimeline} onchange={() => toggleTimeline(p)} /></label
            >
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

{#if editing}
  <ShareLinkDialog
    link={editing}
    onclose={() => (editing = null)}
    onsaved={(l) => (links = links.map((x) => (x.id === l.id ? l : x)))}
  />
{/if}

{#if addingPartner}
  <Pickers
    kind="user"
    exclude={sharedBy.map((p) => p.user.id)}
    onclose={() => (addingPartner = false)}
    onpick={(p) => {
      addingPartner = false;
      if (p.kind === 'user') addPartner(p.user.id);
    }}
  />
{/if}

<style>
  .links {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 960px;
  }
  .links li {
    display: flex;
    gap: 16px;
    align-items: center;
    padding: 12px 12px 12px 12px;
    flex-wrap: wrap;
    border-radius: var(--radius);
    transition: border-color 0.15s;
  }
  .links li:hover {
    border-color: var(--border-strong);
  }
  .thumb {
    width: 76px;
    height: 76px;
    border-radius: 12px;
    overflow: hidden;
    background: var(--bg-soft);
    color: var(--fg-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .info {
    flex: 1;
    min-width: 180px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    font-weight: 600;
  }
  .badges {
    gap: 6px;
    margin-top: 6px;
  }
  .acts {
    gap: 0;
  }
  section {
    margin-bottom: 16px;
    max-width: 760px;
  }
  .section-title {
    margin-bottom: 4px;
  }
  .none {
    margin: 8px 0 0;
  }
  .strong {
    font-weight: 600;
  }
</style>
