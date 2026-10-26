<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import Pickers from '../components/Pickers.svelte';
  import ShareLinkDialog from '../components/ShareLinkDialog.svelte';
  import { api, thumbnailUrl, type PartnerDto, type SharedLinkDto } from '../lib/api';
  import { bumpAssets, confirmDialog, toast, toastError } from '../lib/state.svelte';
  import { router, setQuery } from '../lib/router.svelte';
  import { copyText, formatAbs } from '../lib/utils';
  import { t } from '../lib/i18n';

  const tab = $derived(router.query.get('tab') === 'partners' ? 'partners' : 'links');
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
  <div class="page-header"><h1>{t('navSharing')}</h1></div>
  <div class="tabs">
    <button class:active={tab === 'links'} onclick={() => setQuery({ tab: null })}>{t('sharedLinks')}</button>
    <button class:active={tab === 'partners'} onclick={() => setQuery({ tab: 'partners' })}>{t('partners')}</button>
  </div>

  {#if loading}
    <div class="spinner"></div>
  {:else if tab === 'links'}
    {#if !links.length}
      <div class="empty">{t('noSharedLinks')}</div>
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
                {#if expired(l)}<span class="badge err">{t('expired')}</span>
                {:else if l.expiresAt}<span class="badge">{t('expires')}: {formatAbs(l.expiresAt)}</span>{/if}
                {#if l.hasPassword}<span class="badge"><Icon name="lock" size={12} /> {t('passwordProtected')}</span>{/if}
                {#if l.allowDownload}<span class="badge">{t('download')}</span>{/if}
                {#if l.allowUpload}<span class="badge">{t('upload')}</span>{/if}
              </div>
            </div>
            <div class="row acts">
              <a class="icon-btn" href={l.url} target="_blank" rel="noopener" data-external aria-label="Open"><Icon name="openNew" /></a>
              <button class="icon-btn" onclick={() => copy(l)} title={t('copyLink')} aria-label={t('copyLink')}><Icon name="copy" /></button>
              <button class="icon-btn" onclick={() => (editing = l)} title={t('edit')} aria-label={t('edit')}><Icon name="edit" /></button>
              <button class="icon-btn" onclick={() => remove(l)} title={t('delete')} aria-label={t('delete')}><Icon name="delete" /></button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <section>
      <div class="row head">
        <h2>{t('partnerSharedBy')}</h2>
        <span class="spacer"></span>
        <button class="btn" onclick={() => (addingPartner = true)}><Icon name="add" size={20} /> {t('addPartner')}</button>
      </div>
      {#if !sharedBy.length}<p class="muted">{t('noPartners')}</p>{/if}
      {#each sharedBy as p (p.user.id)}
        <div class="partner row">
          <Icon name="person" />
          <div class="grow"><div>{p.user.name}</div><small class="muted">{p.user.email}</small></div>
          <button class="icon-btn" onclick={() => removePartner(p)} aria-label={t('remove')}><Icon name="close" /></button>
        </div>
      {/each}
    </section>
    <section>
      <div class="row head"><h2>{t('partnerSharedWith')}</h2></div>
      {#if !sharedWith.length}<p class="muted">{t('noPartners')}</p>{/if}
      {#each sharedWith as p (p.user.id)}
        <div class="partner row">
          <Icon name="person" />
          <div class="grow"><div>{p.user.name}</div><small class="muted">{p.user.email}</small></div>
          <label class="check small"><input type="checkbox" checked={p.inTimeline} onchange={() => toggleTimeline(p)} /> {t('showInTimeline')}</label>
        </div>
      {/each}
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
    gap: 10px;
  }
  .links li {
    display: flex;
    gap: 14px;
    align-items: center;
    padding: 10px;
    flex-wrap: wrap;
  }
  .thumb {
    width: 72px;
    height: 72px;
    border-radius: 10px;
    overflow: hidden;
    background: var(--bg-soft);
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
  }
  .title {
    font-weight: 500;
  }
  .badges {
    gap: 4px;
    margin-top: 4px;
  }
  .badge.err {
    color: var(--danger);
  }
  .acts {
    gap: 0;
  }
  section {
    margin-bottom: 28px;
    max-width: 720px;
  }
  .head {
    margin-bottom: 8px;
  }
  .partner {
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
    gap: 14px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
</style>
