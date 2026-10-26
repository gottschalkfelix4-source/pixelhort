<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import Logo from './Logo.svelte';
  import { router, navigate } from '../lib/router.svelte';
  import { auth, logout, prefs, setTheme, ui } from '../lib/state.svelte';
  import { pickFiles } from '../lib/upload.svelte';
  import { formatBytes } from '../lib/utils';
  import { t, type MessageKey } from '../lib/i18n';

  let { children }: { children: Snippet } = $props();

  let query = $state('');
  let userMenu = $state(false);
  let scrollY = $state(0);

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

  const initial = $derived((auth.user?.name || auth.user?.email || '?').slice(0, 1).toUpperCase());

  const usagePct = $derived(
    auth.user?.quotaBytes ? Math.min(100, (auth.user.usageBytes / auth.user.quotaBytes) * 100) : null,
  );
</script>

<svelte:window bind:scrollY />

<header class="topbar" class:scrolled={scrollY > 4}>
  <button class="icon-btn menu-btn" onclick={() => {
      ui.sidebarOpen = !ui.sidebarOpen;
    }} aria-label={t('menu')}><Icon name="menu" /></button>
  <a class="logo" href="/photos">
    <Logo size={32} wordmark />
  </a>
  <form class="search" onsubmit={submitSearch} role="search">
    <Icon name="search" size={22} />
    <input type="search" bind:value={query} placeholder={t('searchPlaceholder')} aria-label={t('search')} />
    <a class="icon-btn filter" href="/search?filter=1" title={t('searchFilters')} aria-label={t('searchFilters')}><Icon name="tune" size={22} /></a>
  </form>
  <span class="spacer"></span>
  <div class="actions">
    <button class="btn upload-btn" onclick={() => pickFiles()}><Icon name="upload" size={20} /> <span class="lbl">{t('upload')}</span></button>
    <button class="icon-btn theme-btn" onclick={cycleTheme} title="{t('theme')}: {ui.theme}" aria-label={t('theme')}>
      <Icon name={ui.theme === 'dark' ? 'moon' : ui.theme === 'light' ? 'sun' : 'contrast'} />
    </button>
    <div class="user-wrap">
      <button class="avatar" class:open={userMenu} onclick={() => (userMenu = !userMenu)} aria-label={auth.user?.name}>
        {initial}
      </button>
      {#if userMenu}
        <div class="menu-backdrop" role="presentation" onclick={() => (userMenu = false)}></div>
        <div class="user-menu">
          <div class="who">
            <span class="avatar big">{initial}</span>
            <strong>{auth.user?.name}</strong>
            <small class="muted">{auth.user?.email}</small>
            <a class="btn small" href="/settings" onclick={() => (userMenu = false)}><Icon name="settings" size={18} /> {t('navSettings')}</a>
          </div>
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
  </div>
</header>

<aside class="sidebar" class:open={ui.sidebarOpen}>
  <div class="sidebar-logo">
    <Logo size={30} wordmark />
  </div>
  <nav>
    {#each visibleNav as item, i (item.href)}
      {#if item.section && !visibleNav[i - 1]?.section}
        <div class="section">{t('navLibrary')}</div>
      {/if}
      <a href={item.href} class:active={isActive(item.href)} onclick={() => {
        ui.sidebarOpen = false;
      }}>
        <Icon name={item.icon} size={22} />
        <span>{t(item.label)}</span>
      </a>
    {/each}
  </nav>
  {#if auth.user}
    <div class="storage">
      <div class="storage-head">
        <Icon name="cloud" size={20} />
        <span>{t('storageSpace')}</span>
      </div>
      {#if usagePct !== null}
        <div class="bar" class:warn={usagePct > 90}><div style="width: {Math.max(2, usagePct)}%"></div></div>
        <small class="muted">{t('storageOf', { used: formatBytes(auth.user.usageBytes), quota: formatBytes(auth.user.quotaBytes) })}</small>
      {:else}
        <small class="muted">{t('storageUsed', { used: formatBytes(auth.user.usageBytes) })}</small>
      {/if}
    </div>
    <div class="server">
      <span class="dot" class:online={!!auth.serverInfo}></span>
      <span>{auth.serverInfo ? t('serverOnline') : t('serverOffline')}</span>
      {#if auth.serverInfo}<span class="spacer"></span><span class="muted">v{auth.serverInfo.version}</span>{/if}
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
    padding: 0 16px 0 12px;
    background: var(--bg);
    border-bottom: 1px solid transparent;
    z-index: 50;
    transition: border-color 0.2s;
  }
  .topbar.scrolled {
    border-bottom-color: var(--border);
  }
  .logo {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--fg);
    font-size: 1.35rem;
    padding-left: 12px;
    width: calc(var(--sidebar-w) - 12px);
    flex-shrink: 0;
  }
  .menu-btn {
    display: none;
  }
  .search {
    flex: 1;
    max-width: 640px;
    display: flex;
    align-items: center;
    gap: 12px;
    background: var(--bg-soft);
    border: 1px solid transparent;
    border-radius: 999px;
    padding: 0 4px 0 18px;
    height: 48px;
    color: var(--fg-muted);
    transition:
      background 0.15s,
      box-shadow 0.15s,
      border-color 0.15s;
  }
  .search:hover {
    border-color: var(--border);
  }
  .search:focus-within {
    background: var(--bg-elev);
    border-color: var(--border);
    box-shadow: var(--shadow);
    color: var(--fg);
  }
  .search input {
    border: none;
    background: transparent;
    outline: none;
    min-height: 0;
    padding: 0;
    box-shadow: none;
  }
  .search input:focus {
    outline: none;
    box-shadow: none;
    background: transparent;
  }
  .search input::-webkit-search-cancel-button {
    display: none;
  }
  .filter {
    color: var(--fg-muted);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .upload-btn {
    margin-right: 4px;
  }
  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    border: none;
    background: linear-gradient(135deg, #3b8cff, #1557c9);
    color: #fff;
    font-weight: 700;
    cursor: pointer;
    box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px transparent;
    transition: box-shadow 0.15s;
  }
  button.avatar:hover,
  button.avatar.open {
    box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px var(--primary);
  }
  .avatar.big {
    width: 72px;
    height: 72px;
    font-size: 1.8rem;
    margin-bottom: 8px;
    cursor: default;
  }
  .user-wrap {
    position: relative;
    margin-left: 4px;
  }
  .user-menu {
    position: absolute;
    right: 0;
    top: 50px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow);
    width: 300px;
    padding: 8px;
    z-index: 70;
    display: flex;
    flex-direction: column;
    animation: drop 0.15s ease-out;
    transform-origin: top right;
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-4px);
    }
  }
  .who {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 20px 12px 18px;
    margin-bottom: 6px;
    border-radius: var(--radius);
    background: var(--bg-soft);
  }
  .who strong {
    font-size: 1.05rem;
  }
  .who small {
    margin-bottom: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .user-menu > a,
  .user-menu > button {
    display: flex;
    gap: 14px;
    align-items: center;
    padding: 10px 14px;
    color: var(--fg);
    border: none;
    background: none;
    cursor: pointer;
    text-align: left;
    border-radius: 10px;
    font-weight: 500;
  }
  .user-menu > a:hover,
  .user-menu > button:hover {
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
    padding: 4px 12px 16px 0;
    overflow-y: auto;
    background: var(--bg);
    z-index: 40;
    display: flex;
    flex-direction: column;
    scrollbar-width: none;
  }
  .sidebar-logo {
    display: none;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
  }
  nav a {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0 24px;
    min-height: 46px;
    border-radius: 0 999px 999px 0;
    color: var(--fg);
    font-weight: 500;
    font-size: 0.93rem;
    transition:
      background 0.15s,
      color 0.15s;
  }
  nav a :global(svg) {
    color: var(--fg-muted);
    transition: color 0.15s;
  }
  nav a:hover {
    background: var(--bg-hover);
  }
  nav a.active {
    background: var(--bg-active);
    color: var(--primary);
    font-weight: 600;
  }
  nav a.active :global(svg) {
    color: var(--primary);
  }
  .section {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--fg-muted);
    padding: 20px 24px 8px;
  }
  .storage {
    margin: 20px 0 0 16px;
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .storage-head {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 600;
    font-size: 0.88rem;
  }
  .storage-head :global(svg) {
    color: var(--primary);
  }
  .storage .bar {
    height: 6px;
    background: var(--bg-soft);
    border-radius: 3px;
    overflow: hidden;
  }
  .storage .bar div {
    height: 100%;
    border-radius: 3px;
    background: var(--primary);
  }
  .storage .bar.warn div {
    background: var(--danger);
  }
  .server {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 0 0 16px;
    padding: 0 16px;
    font-size: 0.78rem;
    font-weight: 500;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
  }
  .dot.online {
    background: #22c55e;
    box-shadow: 0 0 0 3px rgba(34, 197, 94, 0.18);
  }
  .main {
    padding-top: var(--topbar-h);
    margin-left: var(--sidebar-w);
    min-height: 100vh;
  }
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    z-index: 55;
    animation: fade 0.2s;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @media (max-width: 1000px) {
    .menu-btn {
      display: inline-flex;
    }
    .logo {
      width: auto;
      padding-left: 0;
    }
    .sidebar {
      top: 0;
      z-index: 56;
      padding-top: 12px;
      transform: translateX(-100%);
      transition: transform 0.25s ease;
      box-shadow: var(--shadow);
      background: var(--bg-elev);
      border-radius: 0 var(--radius-lg) var(--radius-lg) 0;
    }
    .sidebar.open {
      transform: none;
    }
    .sidebar-logo {
      display: flex;
      align-items: center;
      gap: 10px;
      font-size: 1.3rem;
      padding: 6px 24px 14px;
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
    .logo :global(.wordmark),
    .lbl {
      display: none;
    }
    .upload-btn {
      width: 40px;
      padding: 0;
      border: none;
      margin: 0;
    }
    .upload-btn :global(svg) {
      margin: 0;
    }
    .search {
      height: 42px;
      padding-left: 14px;
      gap: 8px;
    }
    .topbar {
      gap: 4px;
      padding: 0 8px;
    }
    .actions {
      gap: 0;
    }
    .topbar > .spacer,
    .theme-btn {
      display: none;
    }
    .avatar {
      width: 34px;
      height: 34px;
    }
  }
</style>
