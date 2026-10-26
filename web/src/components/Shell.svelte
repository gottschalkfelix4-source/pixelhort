<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import { router, navigate } from '../lib/router.svelte';
  import { auth, logout, prefs, setTheme, ui } from '../lib/state.svelte';
  import { pickFiles } from '../lib/upload.svelte';
  import { formatBytes } from '../lib/utils';
  import { t, type MessageKey } from '../lib/i18n';

  let { children }: { children: Snippet } = $props();

  let query = $state('');
  let userMenu = $state(false);

  $effect(() => {
    if (router.path === '/search') query = router.query.get('q') ?? '';
  });

  const nav: { href: string; icon: IconName; label: MessageKey; section?: 'library' }[] = [
    { href: '/photos', icon: 'photo', label: 'navPhotos' },
    { href: '/explore', icon: 'explore', label: 'navExplore' },
    { href: '/map', icon: 'map', label: 'navMap' },
    { href: '/sharing', icon: 'share', label: 'navSharing' },
    { href: '/albums', icon: 'album', label: 'navAlbums', section: 'library' },
    { href: '/people', icon: 'people', label: 'navPeople', section: 'library' },
    { href: '/tags', icon: 'tag', label: 'navTags', section: 'library' },
    { href: '/favorites', icon: 'heartOutline', label: 'navFavorites', section: 'library' },
    { href: '/archive', icon: 'archive', label: 'navArchive', section: 'library' },
    { href: '/locked', icon: 'lock', label: 'navLocked', section: 'library' },
    { href: '/duplicates', icon: 'duplicate', label: 'navDuplicates', section: 'library' },
    { href: '/trash', icon: 'delete', label: 'navTrash', section: 'library' },
  ];

  const hidden = $derived(
    new Set([
      ...(prefs.value.people.enabled ? [] : ['/people']),
      ...(prefs.value.tags.enabled ? [] : ['/tags']),
      ...(auth.serverInfo?.features.map === false ? ['/map'] : []),
    ]),
  );
  const visibleNav = $derived(nav.filter((n) => !hidden.has(n.href)));

  function isActive(href: string) {
    return router.path === href || router.path.startsWith(href + '/');
  }

  function submitSearch(e: Event) {
    e.preventDefault();
    const q = query.trim();
    if (q) navigate(`/search?q=${encodeURIComponent(q)}`);
  }

  function cycleTheme() {
    setTheme(ui.theme === 'system' ? 'dark' : ui.theme === 'dark' ? 'light' : 'system');
  }

  const usagePct = $derived(
    auth.user?.quotaBytes ? Math.min(100, (auth.user.usageBytes / auth.user.quotaBytes) * 100) : null,
  );
</script>

<header class="topbar">
  <button class="icon-btn menu-btn" onclick={() => {
      ui.sidebarOpen = !ui.sidebarOpen;
    }} aria-label={t('menu')}><Icon name="menu" /></button>
  <a class="logo" href="/photos">
    <svg viewBox="0 0 64 64" width="30" height="30" aria-hidden="true"
      ><rect width="64" height="64" rx="14" fill="#1a73e8" /><path d="M14 46l12-16 9 11 6-7 9 12z" fill="#fff" /><circle
        cx="42"
        cy="20"
        r="6"
        fill="#fbbc04"
      /></svg
    >
    <span>{t('appName')}</span>
  </a>
  <form class="search" onsubmit={submitSearch} role="search">
    <Icon name="search" />
    <input type="search" bind:value={query} placeholder={t('searchPlaceholder')} aria-label={t('search')} />
    <a class="icon-btn" href="/search?filter=1" title={t('searchFilters')} aria-label={t('searchFilters')}><Icon name="tune" /></a>
  </form>
  <span class="spacer"></span>
  <button class="btn text upload-btn" onclick={() => pickFiles()}><Icon name="upload" size={20} /> <span class="lbl">{t('upload')}</span></button>
  <button class="icon-btn" onclick={cycleTheme} title="{t('theme')}: {ui.theme}" aria-label={t('theme')}>
    <Icon name={ui.theme === 'dark' ? 'moon' : ui.theme === 'light' ? 'sun' : 'settings'} />
  </button>
  <div class="user-wrap">
    <button class="avatar" onclick={() => (userMenu = !userMenu)} aria-label={auth.user?.name}>
      {(auth.user?.name || auth.user?.email || '?').slice(0, 1).toUpperCase()}
    </button>
    {#if userMenu}
      <div class="menu-backdrop" role="presentation" onclick={() => (userMenu = false)}></div>
      <div class="user-menu">
        <div class="who">
          <strong>{auth.user?.name}</strong>
          <small class="muted">{auth.user?.email}</small>
        </div>
        <a href="/settings" onclick={() => (userMenu = false)}><Icon name="settings" size={20} /> {t('navSettings')}</a>
        {#if auth.user?.isAdmin}
          <a href="/admin" onclick={() => (userMenu = false)}><Icon name="shield" size={20} /> {t('navAdmin')}</a>
        {/if}
        <button
          onclick={async () => {
            userMenu = false;
            await logout();
            navigate('/login');
          }}><Icon name="logout" size={20} /> {t('logout')}</button
        >
      </div>
    {/if}
  </div>
</header>

<aside class="sidebar" class:open={ui.sidebarOpen}>
  <nav>
    {#each visibleNav as item, i (item.href)}
      {#if item.section && !visibleNav[i - 1]?.section}
        <div class="section">{t('navLibrary')}</div>
      {/if}
      <a href={item.href} class:active={isActive(item.href)} onclick={() => {
        ui.sidebarOpen = false;
      }}>
        <Icon name={item.icon} />
        <span>{t(item.label)}</span>
      </a>
    {/each}
  </nav>
  {#if auth.user}
    <div class="storage">
      {#if usagePct !== null}
        <div class="bar"><div style="width: {usagePct}%"></div></div>
        <small class="muted">{t('storageOf', { used: formatBytes(auth.user.usageBytes), quota: formatBytes(auth.user.quotaBytes) })}</small>
      {:else}
        <small class="muted">{t('storageUsed', { used: formatBytes(auth.user.usageBytes) })}</small>
      {/if}
      {#if auth.serverInfo}<small class="muted">v{auth.serverInfo.version}</small>{/if}
    </div>
  {/if}
</aside>
{#if ui.sidebarOpen}
  <div class="scrim" role="presentation" onclick={() => {
        ui.sidebarOpen = false;
      }}></div>
{/if}

<main class="main">
  {@render children()}
</main>

<style>
  .topbar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: var(--topbar-h);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    z-index: 50;
  }
  .logo {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--fg);
    font-size: 1.3rem;
    width: calc(var(--sidebar-w) - 24px);
    flex-shrink: 0;
  }
  .menu-btn {
    display: none;
  }
  .search {
    flex: 1;
    max-width: 720px;
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-soft);
    border-radius: 24px;
    padding: 0 4px 0 16px;
    height: 46px;
    color: var(--fg-muted);
  }
  .search input {
    border: none;
    background: transparent;
    outline: none;
    min-height: 0;
    padding: 0;
  }
  .search input:focus {
    outline: none;
  }
  .spacer {
    flex: 0.2;
  }
  .avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: none;
    background: var(--primary);
    color: var(--primary-fg);
    font-weight: 600;
    cursor: pointer;
  }
  .user-wrap {
    position: relative;
  }
  .user-menu {
    position: absolute;
    right: 0;
    top: 46px;
    background: var(--bg-elev);
    border-radius: 12px;
    box-shadow: var(--shadow);
    min-width: 240px;
    padding: 8px 0;
    z-index: 70;
    display: flex;
    flex-direction: column;
  }
  .who {
    display: flex;
    flex-direction: column;
    padding: 8px 16px 12px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 6px;
  }
  .user-menu a,
  .user-menu button {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 10px 16px;
    color: var(--fg);
    border: none;
    background: none;
    cursor: pointer;
    text-align: left;
  }
  .user-menu a:hover,
  .user-menu button:hover {
    background: var(--bg-hover);
  }
  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 69;
  }
  .sidebar {
    position: fixed;
    top: var(--topbar-h);
    left: 0;
    bottom: 0;
    width: var(--sidebar-w);
    padding: 8px 12px 16px 0;
    overflow-y: auto;
    background: var(--bg);
    z-index: 40;
    display: flex;
    flex-direction: column;
  }
  nav {
    display: flex;
    flex-direction: column;
    flex: 1;
  }
  nav a {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 0 24px;
    height: 44px;
    border-radius: 0 22px 22px 0;
    color: var(--fg);
    font-weight: 500;
    font-size: 0.92rem;
  }
  nav a:hover {
    background: var(--bg-hover);
  }
  nav a.active {
    background: var(--bg-active);
    color: var(--primary);
  }
  .section {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--fg-muted);
    padding: 16px 24px 6px;
  }
  .storage {
    padding: 16px 24px 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .storage .bar {
    height: 4px;
    background: var(--bg-soft);
    border-radius: 2px;
    overflow: hidden;
  }
  .storage .bar div {
    height: 100%;
    background: var(--primary);
  }
  .main {
    padding-top: var(--topbar-h);
    margin-left: var(--sidebar-w);
    min-height: 100vh;
  }
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 39;
  }

  @media (max-width: 1000px) {
    .menu-btn {
      display: inline-flex;
    }
    .logo {
      width: auto;
    }
    .sidebar {
      transform: translateX(-100%);
      transition: transform 0.2s;
      box-shadow: var(--shadow);
      background: var(--bg-elev);
    }
    .sidebar.open {
      transform: none;
    }
    .main {
      margin-left: 0;
    }
  }
  @media (min-width: 1001px) {
    .scrim {
      display: none;
    }
  }
  @media (max-width: 700px) {
    .logo span,
    .lbl {
      display: none;
    }
    .search {
      height: 40px;
      padding-left: 12px;
    }
    .topbar {
      gap: 4px;
      padding: 0 6px;
    }
    .spacer {
      display: none;
    }
  }
</style>
