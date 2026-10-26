<script lang="ts">
  import type { Component } from 'svelte';
  import Shell from './components/Shell.svelte';
  import Dialogs from './components/Dialogs.svelte';
  import UploadPanel from './components/UploadPanel.svelte';
  import AssetViewer from './components/AssetViewer.svelte';
  import Icon from './components/Icon.svelte';
  import { router, matchRoute, navigate } from './lib/router.svelte';
  import { auth, initAuth, viewer, closeViewer } from './lib/state.svelte';
  import { onUnauthorized } from './lib/api';
  import { addFiles } from './lib/upload.svelte';
  import { t } from './lib/i18n';

  import Login from './pages/Login.svelte';
  import OAuthCallback from './pages/OAuthCallback.svelte';
  import SharedView from './pages/SharedView.svelte';
  import Photos from './pages/Photos.svelte';
  import Explore from './pages/Explore.svelte';
  import Search from './pages/Search.svelte';
  import MapPage from './pages/MapPage.svelte';
  import Sharing from './pages/Sharing.svelte';
  import Albums from './pages/Albums.svelte';
  import AlbumDetail from './pages/AlbumDetail.svelte';
  import People from './pages/People.svelte';
  import PersonDetail from './pages/PersonDetail.svelte';
  import Tags from './pages/Tags.svelte';
  import TagDetail from './pages/TagDetail.svelte';
  import Favorites from './pages/Favorites.svelte';
  import Archive from './pages/Archive.svelte';
  import Locked from './pages/Locked.svelte';
  import Trash from './pages/Trash.svelte';
  import Duplicates from './pages/Duplicates.svelte';
  import Settings from './pages/Settings.svelte';
  import Admin from './pages/Admin.svelte';
  import NotFound from './pages/NotFound.svelte';

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  type AnyComponent = Component<any>;
  interface Route {
    path: string;
    component: AnyComponent;
    public?: boolean;
  }

  const routes: Route[] = [
    { path: '/login', component: Login, public: true },
    { path: '/auth/oauth-callback', component: OAuthCallback, public: true },
    { path: '/s/:key', component: SharedView, public: true },
    { path: '/photos', component: Photos },
    { path: '/explore', component: Explore },
    { path: '/search', component: Search },
    { path: '/map', component: MapPage },
    { path: '/sharing', component: Sharing },
    { path: '/albums', component: Albums },
    { path: '/albums/:id', component: AlbumDetail },
    { path: '/people', component: People },
    { path: '/people/:id', component: PersonDetail },
    { path: '/tags', component: Tags },
    { path: '/tags/:id', component: TagDetail },
    { path: '/favorites', component: Favorites },
    { path: '/archive', component: Archive },
    { path: '/locked', component: Locked },
    { path: '/trash', component: Trash },
    { path: '/duplicates', component: Duplicates },
    { path: '/settings', component: Settings },
    { path: '/admin/:tab?', component: Admin },
  ];

  const matched = $derived.by(() => {
    for (const r of routes) {
      const params = matchRoute(r.path, router.path);
      if (params) return { route: r, params };
    }
    return null;
  });

  initAuth();
  onUnauthorized(() => {
    if (!auth.user) return;
    auth.user = null;
    closeViewer();
  });

  // Redirects
  $effect(() => {
    if (!auth.ready) return;
    const path = router.path;
    if (path === '/' || path === '') {
      navigate(auth.user ? '/photos' : '/login', { replace: true });
      return;
    }
    if (matched && !matched.route.public && !auth.user) {
      navigate(`/login?redirect=${encodeURIComponent(path + location.search)}`, { replace: true });
    }
  });

  // Close viewer on navigation
  $effect(() => {
    void router.path;
    closeViewer();
  });

  // ── Global drag & drop upload ──
  let dragging = $state(false);
  let dragDepth = 0;
  const dropEnabled = $derived(!!auth.user && !!matched && !matched.route.public);

  function hasFiles(e: DragEvent) {
    return !!e.dataTransfer && Array.from(e.dataTransfer.types).includes('Files');
  }
  function onDragEnter(e: DragEvent) {
    if (!dropEnabled || !hasFiles(e)) return;
    e.preventDefault();
    dragDepth++;
    dragging = true;
  }
  function onDragOver(e: DragEvent) {
    if (!dropEnabled || !hasFiles(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
  }
  function onDragLeave() {
    if (!dragging) return;
    dragDepth = Math.max(0, dragDepth - 1);
    if (!dragDepth) dragging = false;
  }
  function onDrop(e: DragEvent) {
    if (!dropEnabled || !hasFiles(e)) return;
    e.preventDefault();
    dragging = false;
    dragDepth = 0;
    const files = e.dataTransfer?.files;
    if (!files?.length) return;
    const albumMatch = matchRoute('/albums/:id', router.path);
    addFiles(files, { albumId: albumMatch?.id });
  }
</script>

<svelte:window ondragenter={onDragEnter} ondragover={onDragOver} ondragleave={onDragLeave} ondrop={onDrop} />

{#if !auth.ready}
  <div class="boot"><div class="spinner"></div></div>
{:else if matched?.route.public}
  {@const Page = matched.route.component}
  <Page params={matched.params} />
{:else if !auth.user}
  <div class="boot"><div class="spinner"></div></div>
{:else}
  <Shell>
    {#if matched}
      {@const Page = matched.route.component}
      {#key router.path}
        <Page params={matched.params} />
      {/key}
    {:else}
      <NotFound />
    {/if}
  </Shell>
{/if}

{#if viewer.ctx}
  {#key viewer.ctx}
    <AssetViewer />
  {/key}
{/if}

<UploadPanel />
<Dialogs />

{#if dragging}
  <div class="dropzone">
    <div class="drop-inner">
      <Icon name="upload" size={56} />
      <p>{t('dropHere')}</p>
    </div>
  </div>
{/if}

<style>
  .boot {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .dropzone {
    position: fixed;
    inset: 0;
    z-index: 2000;
    background: color-mix(in srgb, var(--primary) 14%, transparent);
    backdrop-filter: blur(2px);
    border: 3px dashed var(--primary);
    border-radius: 16px;
    margin: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }
  .drop-inner {
    background: var(--bg-elev);
    color: var(--primary);
    padding: 36px 56px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    text-align: center;
    font-size: 1.1rem;
    font-weight: 500;
  }
</style>
