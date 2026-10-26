<script lang="ts" module>
  export type PickerKind = 'album' | 'tag' | 'person' | 'user';
</script>

<script lang="ts">
  import Modal from './Modal.svelte';
  import Icon from './Icon.svelte';
  import {
    api,
    personThumbnailUrl,
    thumbnailUrl,
    type AlbumDto,
    type PersonDto,
    type TagDto,
    type UserSummary,
  } from '../lib/api';
  import { auth, toastError } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  type Picked =
    | { kind: 'album'; album: AlbumDto }
    | { kind: 'tag'; tag: TagDto }
    | { kind: 'person'; person: PersonDto | null }
    | { kind: 'user'; user: UserSummary };

  let {
    kind,
    title,
    exclude = [],
    allowCreate = true,
    onpick,
    onclose,
  }: {
    kind: PickerKind;
    title?: string;
    exclude?: string[];
    allowCreate?: boolean;
    onpick: (p: Picked) => void;
    onclose: () => void;
  } = $props();

  let query = $state('');
  let loading = $state(true);
  let albums = $state<AlbumDto[]>([]);
  let tags = $state<TagDto[]>([]);
  let people = $state<PersonDto[]>([]);
  let users = $state<UserSummary[]>([]);

  $effect(() => {
    load();
  });

  async function load() {
    loading = true;
    try {
      if (kind === 'album') {
        const all = await api.albums();
        const me = auth.user?.id;
        albums = all.filter(
          (a) => a.ownerId === me || a.albumUsers.some((u) => u.user.id === me && u.role === 'editor'),
        );
      } else if (kind === 'tag') tags = await api.tags();
      else if (kind === 'person') people = (await api.people(true)).people;
      else users = (await api.users()).filter((u) => u.id !== auth.user?.id);
    } catch (e) {
      toastError(e);
    } finally {
      loading = false;
    }
  }

  const q = $derived(query.trim().toLowerCase());
  const fAlbums = $derived(albums.filter((a) => !exclude.includes(a.id) && a.albumName.toLowerCase().includes(q)));
  const fTags = $derived(tags.filter((x) => !exclude.includes(x.id) && x.value.toLowerCase().includes(q)));
  const fPeople = $derived(people.filter((p) => !exclude.includes(p.id) && (p.name || '').toLowerCase().includes(q)));
  const fUsers = $derived(
    users.filter((u) => !exclude.includes(u.id) && (u.name + ' ' + u.email).toLowerCase().includes(q)),
  );

  async function createNew() {
    const name = query.trim();
    try {
      if (kind === 'album') {
        const album = await api.createAlbum({ albumName: name || t('untitledAlbum') });
        onpick({ kind: 'album', album });
      } else if (kind === 'tag' && name) {
        const tag = await api.createTag(name);
        onpick({ kind: 'tag', tag });
      } else if (kind === 'person') {
        onpick({ kind: 'person', person: null });
      }
    } catch (e) {
      toastError(e);
    }
  }

  const defaultTitle = $derived(
    kind === 'album' ? t('chooseAlbum') : kind === 'tag' ? t('chooseTag') : kind === 'person' ? t('choosePerson') : t('chooseUser'),
  );
</script>

<Modal title={title ?? defaultTitle} {onclose} width={460}>
  <!-- svelte-ignore a11y_autofocus -->
  <input
    type="search"
    bind:value={query}
    placeholder={kind === 'tag' ? t('tagValueHint') : t('search')}
    autofocus
    onkeydown={(e) => e.key === 'Enter' && kind === 'tag' && !fTags.length && createNew()}
  />
  <div class="list">
    {#if loading}
      <div class="spinner"></div>
    {:else}
      {#if allowCreate && kind !== 'user'}
        {#if kind !== 'tag' || (q && !tags.some((x) => x.value.toLowerCase() === q))}
          <button class="item" onclick={createNew}>
            <span class="thumb add"><Icon name="add" /></span>
            <span class="label">
              {#if kind === 'album'}{t('newAlbum')}{query ? `: ${query}` : ''}
              {:else if kind === 'tag'}{t('newTag')}: {query}
              {:else}{t('newPerson')}{/if}
            </span>
          </button>
        {/if}
      {/if}
      {#if kind === 'album'}
        {#each fAlbums as a (a.id)}
          <button class="item" onclick={() => onpick({ kind: 'album', album: a })}>
            <span class="thumb">
              {#if a.albumThumbnailAssetId}<img src={thumbnailUrl(a.albumThumbnailAssetId)} alt="" />{:else}<Icon
                  name="album"
                />{/if}
            </span>
            <span class="label">{a.albumName}<small class="muted">{t('photosCount', { n: a.assetCount })}</small></span>
          </button>
        {/each}
      {:else if kind === 'tag'}
        {#each fTags as x (x.id)}
          <button class="item" onclick={() => onpick({ kind: 'tag', tag: x })}>
            <span class="thumb" style="color: {x.color || 'var(--fg-muted)'}"><Icon name="tag" /></span>
            <span class="label">{x.value}</span>
          </button>
        {/each}
      {:else if kind === 'person'}
        {#each fPeople as p (p.id)}
          <button class="item" onclick={() => onpick({ kind: 'person', person: p })}>
            <span class="thumb round"><img src={personThumbnailUrl(p.id)} alt="" /></span>
            <span class="label">{p.name || t('unnamedPerson')}</span>
          </button>
        {/each}
      {:else}
        {#each fUsers as u (u.id)}
          <button class="item" onclick={() => onpick({ kind: 'user', user: u })}>
            <span class="thumb round"><Icon name="person" /></span>
            <span class="label">{u.name}<small class="muted">{u.email}</small></span>
          </button>
        {/each}
      {/if}
    {/if}
  </div>
</Modal>

<style>
  .list {
    margin-top: 12px;
    display: flex;
    flex-direction: column;
    max-height: 55vh;
    overflow-y: auto;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px;
    border: none;
    background: none;
    border-radius: 8px;
    cursor: pointer;
    text-align: left;
  }
  .item:hover {
    background: var(--bg-hover);
  }
  .thumb {
    width: 48px;
    height: 48px;
    border-radius: 8px;
    background: var(--bg-soft);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    flex-shrink: 0;
    color: var(--fg-muted);
  }
  .thumb.round {
    border-radius: 50%;
  }
  .thumb.add {
    color: var(--primary);
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .label {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
</style>
