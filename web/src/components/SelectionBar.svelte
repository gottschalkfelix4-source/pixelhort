<script lang="ts" module>
  export type SelectionContext = 'timeline' | 'archive' | 'favorites' | 'album' | 'trash' | 'locked' | 'search' | 'person' | 'tag';
</script>

<script lang="ts">
  import Icon from './Icon.svelte';
  import Pickers from './Pickers.svelte';
  import ShareLinkDialog from './ShareLinkDialog.svelte';
  import { api, type Uuid, type Visibility } from '../lib/api';
  import type { Selection } from '../lib/selection.svelte';
  import { auth, bumpAssets, confirmDialog, toast, toastError } from '../lib/state.svelte';
  import { downloadAssets } from '../lib/utils';
  import { t } from '../lib/i18n';

  let {
    selection,
    context = 'timeline',
    albumId = null,
    canEditAlbum = false,
    onchanged,
    onsetcover,
  }: {
    selection: Selection;
    context?: SelectionContext;
    albumId?: Uuid | null;
    canEditAlbum?: boolean;
    onchanged?: () => void;
    onsetcover?: (id: Uuid) => void;
  } = $props();

  let picker = $state<'album' | 'tag' | null>(null);
  let sharing = $state<Uuid[] | null>(null);
  let menuOpen = $state(false);

  const me = $derived(auth.user?.id);
  const items = $derived(selection.list());
  const owned = $derived(items.filter((a) => a.ownerId === me));
  const allFav = $derived(items.length > 0 && items.every((a) => a.isFavorite));
  const allOwned = $derived(owned.length === items.length);

  function done(msg?: string) {
    if (msg) toast(msg, 'success');
    selection.clear();
    bumpAssets();
    onchanged?.();
  }

  async function run(fn: () => Promise<unknown>, msg?: string) {
    menuOpen = false;
    try {
      await fn();
      done(msg);
    } catch (e) {
      toastError(e);
    }
  }

  const ids = () => owned.map((a) => a.id);

  const favorite = () => run(() => api.updateAssets({ ids: ids(), isFavorite: !allFav }));
  const setVisibility = (v: Visibility, msg: string) => run(() => api.updateAssets({ ids: ids(), visibility: v }), msg);

  async function moveToLocked() {
    menuOpen = false;
    if (!auth.user?.hasPin) {
      try {
        const s = await api.session();
        if (!s.hasPin) return toast(t('lockedNeedsPin'), 'error');
      } catch {
        return toast(t('lockedNeedsPin'), 'error');
      }
    }
    setVisibility('locked', t('movedToLocked'));
  }

  async function trash() {
    menuOpen = false;
    if (!(await confirmDialog(t('moveToTrash'), { message: t('trashConfirm', { n: owned.length }), confirmLabel: t('moveToTrash'), danger: true })))
      return;
    run(() => api.deleteAssets(ids()), t('movedToTrash'));
  }
  async function deleteForever() {
    if (!(await confirmDialog(t('deletePermanently'), { message: t('deleteForeverConfirm', { n: owned.length }), confirmLabel: t('delete'), danger: true })))
      return;
    run(() => api.deleteAssets(ids(), true));
  }
  const restore = () => run(() => api.restoreTrash(ids()), t('restore'));

  async function removeFromAlbum() {
    if (!albumId) return;
    run(() => api.removeAlbumAssets(albumId!, selection.ids()));
  }

  function download() {
    menuOpen = false;
    downloadAssets(items);
  }
</script>

{#if selection.active}
  <div class="selbar">
    <button class="icon-btn" onclick={() => selection.clear()} aria-label={t('clearSelection')}><Icon name="close" /></button>
    <span class="count">{t('selected', { n: selection.size })}</span>
    <span class="spacer"></span>

    {#if context === 'trash'}
      <button class="btn text" onclick={restore}><Icon name="restore" size={20} /> <span class="lbl">{t('restore')}</span></button>
      <button class="btn text danger" onclick={deleteForever}><Icon name="delete" size={20} /> <span class="lbl">{t('deletePermanently')}</span></button>
    {:else}
      <button class="icon-btn" title={t('createSharedLink')} aria-label={t('createSharedLink')} onclick={() => (sharing = selection.ids())}>
        <Icon name="share" />
      </button>
      {#if context !== 'locked'}
        <button class="icon-btn" title={t('addToAlbum')} aria-label={t('addToAlbum')} onclick={() => (picker = 'album')}><Icon name="add" /></button>
      {/if}
      {#if owned.length}
        <button class="icon-btn" title={allFav ? t('unfavorite') : t('favorite')} aria-label={t('favorite')} onclick={favorite}>
          <Icon name={allFav ? 'heart' : 'heartOutline'} />
        </button>
        <button class="icon-btn" title={t('moveToTrash')} aria-label={t('moveToTrash')} onclick={trash}><Icon name="delete" /></button>
      {/if}
      <div class="menu-wrap">
        <button class="icon-btn" aria-label={t('more')} onclick={() => (menuOpen = !menuOpen)}><Icon name="moreVert" /></button>
        {#if menuOpen}
          <div class="menu" role="menu">
            <button onclick={download}><Icon name="download" size={20} /> {t('download')}</button>
            {#if owned.length}
              <button
                onclick={() => {
                  menuOpen = false;
                  picker = 'tag';
                }}><Icon name="tag" size={20} /> {t('addTag')}</button
              >
              {#if context === 'archive'}
                <button onclick={() => setVisibility('timeline', t('unarchived'))}><Icon name="unarchive" size={20} /> {t('unarchive')}</button>
              {:else if context === 'locked'}
                <button onclick={() => setVisibility('timeline', t('movedFromLocked'))}><Icon name="lockOpen" size={20} /> {t('moveOutOfLocked')}</button>
              {:else}
                <button onclick={() => setVisibility('archive', t('archived'))}><Icon name="archive" size={20} /> {t('archive')}</button>
                <button onclick={moveToLocked}><Icon name="lock" size={20} /> {t('moveToLocked')}</button>
              {/if}
            {/if}
            {#if context === 'album' && canEditAlbum}
              <button onclick={removeFromAlbum}><Icon name="close" size={20} /> {t('removeFromAlbum')}</button>
              {#if selection.size === 1 && onsetcover}
                <button
                  onclick={() => {
                    menuOpen = false;
                    onsetcover?.(selection.ids()[0]);
                    selection.clear();
                  }}><Icon name="photo" size={20} /> {t('setAsCover')}</button
                >
              {/if}
            {/if}
            {#if !allOwned}
              <div class="hint muted small">{owned.length}/{items.length}</div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

{#if picker === 'album'}
  <Pickers
    kind="album"
    exclude={albumId ? [albumId] : []}
    onclose={() => (picker = null)}
    onpick={async (p) => {
      picker = null;
      if (p.kind !== 'album') return;
      try {
        const res = await api.addAlbumAssets(p.album.id, selection.ids());
        const ok = res.filter((r) => r.success).length;
        const dup = res.filter((r) => r.error === 'duplicate').length;
        toast(t('addedToAlbum', { n: ok, album: p.album.albumName }) + (dup ? ` · ${t('alreadyInAlbum', { n: dup })}` : ''), 'success');
        selection.clear();
        bumpAssets();
      } catch (e) {
        toastError(e);
      }
    }}
  />
{:else if picker === 'tag'}
  <Pickers
    kind="tag"
    onclose={() => (picker = null)}
    onpick={async (p) => {
      picker = null;
      if (p.kind !== 'tag') return;
      run(() => api.tagAssets(p.tag.id, ids()), t('tagged', { tag: p.tag.value }));
    }}
  />
{/if}

{#if sharing}
  <ShareLinkDialog
    assetIds={sharing}
    onclose={() => {
      sharing = null;
      selection.clear();
    }}
  />
{/if}

{#if menuOpen}
  <div class="menu-backdrop" role="presentation" onclick={() => (menuOpen = false)}></div>
{/if}

<style>
  .selbar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: var(--topbar-h);
    z-index: 60;
    background: var(--bg-elev);
    box-shadow: var(--shadow);
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 12px;
  }
  .count {
    font-weight: 500;
    font-size: 1.05rem;
    margin-left: 8px;
    white-space: nowrap;
  }
  .menu-wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 44px;
    background: var(--bg-elev);
    border-radius: 8px;
    box-shadow: var(--shadow);
    padding: 6px 0;
    min-width: 240px;
    z-index: 62;
    display: flex;
    flex-direction: column;
  }
  .menu button {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 10px 16px;
    border: none;
    background: none;
    cursor: pointer;
    text-align: left;
    color: var(--fg);
  }
  .menu button:hover {
    background: var(--bg-hover);
  }
  .hint {
    padding: 6px 16px;
  }
  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 59;
  }
  @media (max-width: 600px) {
    .lbl {
      display: none;
    }
  }
</style>
