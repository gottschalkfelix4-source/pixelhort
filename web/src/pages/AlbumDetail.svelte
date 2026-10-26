<script lang="ts">
  import AssetGrid from '../components/AssetGrid.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import Timeline from '../components/Timeline.svelte';
  import Modal from '../components/Modal.svelte';
  import Pickers from '../components/Pickers.svelte';
  import ShareLinkDialog from '../components/ShareLinkDialog.svelte';
  import Icon from '../components/Icon.svelte';
  import { api, type ActivityDto, type AlbumDto, type AlbumRole } from '../lib/api';
  import { Selection } from '../lib/selection.svelte';
  import { auth, bumpAssets, confirmDialog, toast, toastError, ui } from '../lib/state.svelte';
  import { navigate } from '../lib/router.svelte';
  import { pickFiles } from '../lib/upload.svelte';
  import { downloadAssets, formatAbs, formatLocal } from '../lib/utils';
  import { t } from '../lib/i18n';

  let { params }: { params: { id: string } } = $props();

  const selection = new Selection();
  let album = $state<AlbumDto | null>(null);
  let error = $state('');
  let name = $state('');
  let description = $state('');
  let adding = $state(false);
  const pickSelection = new Selection();
  let usersOpen = $state(false);
  let userPicker = $state(false);
  let linkOpen = $state(false);
  let activityOpen = $state(false);
  let activities = $state<ActivityDto[]>([]);
  let comment = $state('');

  const isOwner = $derived(album?.ownerId === auth.user?.id);
  const myRole = $derived(album?.albumUsers.find((u) => u.user.id === auth.user?.id)?.role);
  const canEdit = $derived(isOwner || myRole === 'editor');
  const sorted = $derived.by(() => {
    const list = [...(album?.assets ?? [])];
    const dir = album?.order === 'asc' ? 1 : -1;
    list.sort((a, b) => (a.localDateTime < b.localDateTime ? -dir : a.localDateTime > b.localDateTime ? dir : 0));
    return list;
  });

  async function load() {
    try {
      album = await api.album(params.id);
      name = album.albumName;
      description = album.description ?? '';
      if (album.shared) loadActivities();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  $effect(() => {
    void ui.assetsVersion;
    load();
  });

  async function loadActivities() {
    try {
      activities = await api.activities(params.id);
    } catch {
      activities = [];
    }
  }

  async function patch(p: Parameters<typeof api.updateAlbum>[1]) {
    try {
      const res = await api.updateAlbum(params.id, p);
      if (album) album = { ...res, assets: album.assets };
    } catch (e) {
      toastError(e);
    }
  }

  function saveName() {
    if (album && name !== album.albumName) patch({ albumName: name });
  }
  function saveDescription() {
    if (album && description !== (album.description ?? '')) patch({ description });
  }

  async function addSelected() {
    const ids = pickSelection.ids();
    adding = false;
    pickSelection.clear();
    if (!ids.length) return;
    try {
      const res = await api.addAlbumAssets(params.id, ids);
      toast(t('addedToAlbum', { n: res.filter((r) => r.success).length, album: album?.albumName ?? '' }), 'success');
      load();
    } catch (e) {
      toastError(e);
    }
  }

  async function remove() {
    if (!album) return;
    if (isOwner) {
      if (!(await confirmDialog(t('deleteAlbum'), { message: t('deleteAlbumConfirm', { name: album.albumName }), danger: true, confirmLabel: t('delete') })))
        return;
      try {
        await api.deleteAlbum(album.id);
        navigate('/albums');
      } catch (e) {
        toastError(e);
      }
    } else {
      if (!(await confirmDialog(t('leaveAlbum'), { danger: true }))) return;
      try {
        await api.removeAlbumUser(album.id, 'me');
        navigate('/albums');
      } catch (e) {
        toastError(e);
      }
    }
  }

  async function setRole(userId: string, role: AlbumRole) {
    try {
      const res = await api.addAlbumUsers(params.id, [{ userId, role }]);
      if (album) album = { ...res, assets: album.assets };
    } catch (e) {
      toastError(e);
    }
  }
  async function removeUser(userId: string) {
    try {
      await api.removeAlbumUser(params.id, userId);
      load();
    } catch (e) {
      toastError(e);
    }
  }

  const myLike = $derived(activities.find((a) => a.type === 'like' && !a.assetId && a.user.id === auth.user?.id));
  async function toggleLike() {
    try {
      if (myLike) await api.deleteActivity(myLike.id);
      else await api.addActivity(params.id, { type: 'like' });
      loadActivities();
    } catch (e) {
      toastError(e);
    }
  }
  async function sendComment(e: Event) {
    e.preventDefault();
    if (!comment.trim()) return;
    try {
      await api.addActivity(params.id, { type: 'comment', comment: comment.trim() });
      comment = '';
      loadActivities();
    } catch (err) {
      toastError(err);
    }
  }
  async function deleteActivity(id: string) {
    try {
      await api.deleteActivity(id);
      loadActivities();
    } catch (e) {
      toastError(e);
    }
  }

  function range() {
    if (!album?.startDate) return '';
    const s = formatLocal(album.startDate, { dateStyle: 'medium' });
    const e = album.endDate ? formatLocal(album.endDate, { dateStyle: 'medium' }) : s;
    return s === e ? s : `${s} – ${e}`;
  }
</script>

<div class="layout">
  <div class="page main-col">
    {#if error}
      <div class="empty error-text">{error}</div>
    {:else if !album}
      <div class="spinner"></div>
    {:else}
      <div class="page-header">
        <a class="icon-btn" href="/albums" aria-label={t('back')}><Icon name="back" /></a>
        <span class="spacer"></span>
        {#if canEdit}
          <button class="btn" onclick={() => (adding = true)}><Icon name="add" size={20} /> {t('addPhotos')}</button>
          <button class="icon-btn" title={t('upload')} aria-label={t('upload')} onclick={() => pickFiles({ albumId: album!.id })}
            ><Icon name="upload" /></button
          >
        {/if}
        {#if isOwner}
          <button class="icon-btn" title={t('shareAlbum')} aria-label={t('shareAlbum')} onclick={() => (usersOpen = true)}><Icon name="people" /></button>
          <button class="icon-btn" title={t('createSharedLink')} aria-label={t('createSharedLink')} onclick={() => (linkOpen = true)}
            ><Icon name="link" /></button
          >
          <button
            class="icon-btn"
            title={album.order === 'asc' ? t('sortOldestFirst') : t('sortNewestFirst')}
            aria-label={t('sortNewestFirst')}
            onclick={() => patch({ order: album!.order === 'asc' ? 'desc' : 'asc' })}><Icon name="sort" /></button
          >
        {/if}
        {#if album.shared}
          <button class="icon-btn" class:active={activityOpen} title={t('activity')} aria-label={t('activity')} onclick={() => (activityOpen = !activityOpen)}
            ><Icon name="comment" /></button
          >
        {/if}
        {#if sorted.length}
          <button class="icon-btn" title={t('downloadAll')} aria-label={t('downloadAll')} onclick={() => downloadAssets(sorted)}><Icon name="download" /></button>
        {/if}
        <button class="icon-btn" title={isOwner ? t('deleteAlbum') : t('leaveAlbum')} aria-label={t('delete')} onclick={remove}
          ><Icon name={isOwner ? 'delete' : 'logout'} /></button
        >
      </div>

      <div class="album-head">
        {#if canEdit}
          <input class="title-input" bind:value={name} onblur={saveName} onkeydown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()} placeholder={t('untitledAlbum')} />
          <textarea class="desc-input" rows="1" bind:value={description} onblur={saveDescription} placeholder={t('addDescription')}></textarea>
        {:else}
          <h1>{album.albumName}</h1>
          {#if album.description}<p>{album.description}</p>{/if}
        {/if}
        <div class="muted small">
          {range()}{range() ? ' · ' : ''}{t('photosCount', { n: album.assetCount })}
          {#if album.albumUsers.length}
            · {album.owner.name}, {album.albumUsers.map((u) => u.user.name).join(', ')}
          {/if}
        </div>
      </div>

      {#if !sorted.length}
        <div class="empty">{t('albumEmpty')}</div>
      {:else}
        <AssetGrid assets={sorted} {selection} groupByDay={false} onchange={load} />
      {/if}
    {/if}
  </div>

  {#if activityOpen && album}
    <aside class="activity">
      <header class="row">
        <h2>{t('activity')}</h2>
        <span class="spacer"></span>
        <button class="icon-btn" class:active={!!myLike} onclick={toggleLike} aria-label={t('like')}><Icon name="thumbUp" /></button>
        <button class="icon-btn" onclick={() => (activityOpen = false)} aria-label={t('close')}><Icon name="close" /></button>
      </header>
      <ul>
        {#each activities as a (a.id)}
          <li>
            <div class="row">
              <strong>{a.user.name}</strong>
              <span class="muted small">{formatAbs(a.createdAt)}</span>
              <span class="spacer"></span>
              {#if a.user.id === auth.user?.id || isOwner}
                <button class="icon-btn small-btn" onclick={() => deleteActivity(a.id)} aria-label={t('delete')}><Icon name="close" size={16} /></button>
              {/if}
            </div>
            {#if a.type === 'like'}
              <div class="muted"><Icon name="thumbUp" size={16} /> {t('like')}</div>
            {:else}
              <div>{a.comment}</div>
            {/if}
          </li>
        {/each}
      </ul>
      <form class="row" onsubmit={sendComment}>
        <input type="text" bind:value={comment} placeholder={t('writeComment')} />
        <button class="icon-btn" type="submit" aria-label="Send"><Icon name="send" /></button>
      </form>
    </aside>
  {/if}
</div>

<SelectionBar
  {selection}
  context="album"
  albumId={album?.id}
  canEditAlbum={canEdit}
  onchanged={load}
  onsetcover={(id) => patch({ albumThumbnailAssetId: id }).then(() => toast(t('coverSet'), 'success'))}
/>

{#if adding}
  <Modal title={t('addPhotos')} onclose={() => ((adding = false), pickSelection.clear())} width={1100}>
    <div class="picker">
      <Timeline selection={pickSelection} pickMode scrubber={false} />
    </div>
    {#snippet footer()}
      <span class="muted">{t('selected', { n: pickSelection.size })}</span>
      <span class="spacer"></span>
      <button class="btn text" onclick={() => ((adding = false), pickSelection.clear())}>{t('cancel')}</button>
      <button class="btn primary" disabled={!pickSelection.size} onclick={addSelected}>{t('add')}</button>
    {/snippet}
  </Modal>
{/if}

{#if usersOpen && album}
  <Modal title={t('shareAlbum')} onclose={() => (usersOpen = false)}>
    <ul class="users">
      <li class="row">
        <Icon name="person" />
        <div class="grow"><div>{album.owner.name}</div><small class="muted">{album.owner.email}</small></div>
        <span class="muted small">{t('owner')}</span>
      </li>
      {#each album.albumUsers as u (u.user.id)}
        <li class="row">
          <Icon name="person" />
          <div class="grow"><div>{u.user.name}</div><small class="muted">{u.user.email}</small></div>
          <select value={u.role} onchange={(e) => setRole(u.user.id, (e.currentTarget as HTMLSelectElement).value as AlbumRole)}>
            <option value="editor">{t('roleEditor')}</option>
            <option value="viewer">{t('roleViewer')}</option>
          </select>
          <button class="icon-btn" onclick={() => removeUser(u.user.id)} aria-label={t('remove')}><Icon name="close" /></button>
        </li>
      {/each}
    </ul>
    {#snippet footer()}
      <button class="btn" onclick={() => (userPicker = true)}><Icon name="add" size={20} /> {t('add')}</button>
      <button class="btn primary" onclick={() => (usersOpen = false)}>{t('done')}</button>
    {/snippet}
  </Modal>
{/if}

{#if userPicker && album}
  <Pickers
    kind="user"
    exclude={[album.ownerId, ...album.albumUsers.map((u) => u.user.id)]}
    onclose={() => (userPicker = false)}
    onpick={(p) => {
      userPicker = false;
      if (p.kind === 'user') setRole(p.user.id, 'editor');
    }}
  />
{/if}

{#if linkOpen && album}
  <ShareLinkDialog albumId={album.id} onclose={() => (linkOpen = false)} onsaved={() => bumpAssets()} />
{/if}

<style>
  .layout {
    display: flex;
    min-height: calc(100vh - var(--topbar-h));
  }
  .main-col {
    flex: 1;
    min-width: 0;
  }
  .album-head {
    margin: 8px 0 20px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .title-input {
    font-size: 2rem;
    font-weight: 400;
    border: none;
    border-bottom: 1px solid transparent;
    border-radius: 0;
    padding: 4px 0;
    background: transparent;
  }
  .title-input:hover,
  .title-input:focus {
    border-bottom-color: var(--border);
    outline: none;
  }
  .desc-input {
    border: none;
    padding: 4px 0;
    background: transparent;
    resize: none;
    min-height: 32px;
    color: var(--fg-muted);
  }
  .desc-input:focus {
    outline: none;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
  }
  .activity {
    width: 340px;
    border-left: 1px solid var(--border);
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    position: sticky;
    top: var(--topbar-h);
    height: calc(100vh - var(--topbar-h));
    background: var(--bg);
  }
  .activity ul {
    flex: 1;
    overflow-y: auto;
    list-style: none;
    padding: 0;
    margin: 8px 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .small-btn {
    width: 28px;
    height: 28px;
  }
  .picker {
    min-height: 60vh;
  }
  .users {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .users select {
    width: auto;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  @media (max-width: 900px) {
    .activity {
      position: fixed;
      right: 0;
      top: var(--topbar-h);
      bottom: 0;
      z-index: 45;
      width: min(340px, 100vw);
      box-shadow: var(--shadow);
    }
  }
</style>
