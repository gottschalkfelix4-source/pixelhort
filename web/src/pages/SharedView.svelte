<script lang="ts">
  import AssetGrid from '../components/AssetGrid.svelte';
  import Icon from '../components/Icon.svelte';
  import Logo from '../components/Logo.svelte';
  import { api, ApiError, type AssetDto, type ShareAccess, type SharedLinkDto } from '../lib/api';
  import { auth, setTheme, toast, ui } from '../lib/state.svelte';
  import { addFiles, pickFiles } from '../lib/upload.svelte';
  import { downloadAssets, formatLocal } from '../lib/utils';
  import { t } from '../lib/i18n';

  let { params }: { params: { key: string } } = $props();

  let link = $state<SharedLinkDto | null>(null);
  let assets = $state<AssetDto[]>([]);
  let needPassword = $state(false);
  let error = $state('');
  let password = $state('');
  let busy = $state(false);
  let shareToken = $state<string | undefined>(readToken());

  function readToken(): string | undefined {
    try {
      return sessionStorage.getItem(`ph_share_${params.key}`) ?? undefined;
    } catch {
      return undefined;
    }
  }

  const access = $derived<ShareAccess>({ key: params.key, shareToken });

  async function load() {
    error = '';
    try {
      const r = await api.sharedLinkMe(access);
      link = r.link;
      assets = r.assets;
      needPassword = false;
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) {
        needPassword = true;
        if (shareToken && e.body?.message === 'password_required') shareToken = undefined;
      } else {
        error = t('shareNotFound');
      }
    }
  }

  $effect(() => {
    void ui.assetsVersion;
    void access;
    load();
  });

  async function login(e: Event) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      const r = await api.sharedLinkLogin(params.key, password);
      shareToken = r.shareToken;
      try {
        sessionStorage.setItem(`ph_share_${params.key}`, r.shareToken);
      } catch {
        /* ignore */
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  const title = $derived(link?.album?.albumName || link?.description || t('sharedWithYou'));
  const sorted = $derived([...assets].sort((a, b) => (a.localDateTime < b.localDateTime ? 1 : -1)));
  const range = $derived.by(() => {
    if (!sorted.length) return '';
    const s = formatLocal(sorted[sorted.length - 1].localDateTime, { dateStyle: 'medium' });
    const e = formatLocal(sorted[0].localDateTime, { dateStyle: 'medium' });
    return s === e ? s : `${s} – ${e}`;
  });

  function upload() {
    pickFiles({ share: access });
  }

  let dragging = $state(false);
  function onDrop(e: DragEvent) {
    if (!link?.allowUpload) return;
    e.preventDefault();
    e.stopPropagation();
    dragging = false;
    if (e.dataTransfer?.files.length) addFiles(e.dataTransfer.files, { share: access });
  }
</script>

<div
  class="share"
  role="presentation"
  ondragover={(e) => {
    if (link?.allowUpload) {
      e.preventDefault();
      dragging = true;
    }
  }}
  ondragleave={() => (dragging = false)}
  ondrop={onDrop}
  class:dragging
>
  <header class="top">
    <a class="logo" href={auth.user ? '/photos' : '/login'}>
      <Logo size={28} wordmark />
    </a>
    <span class="spacer"></span>
    {#if link?.allowUpload}
      <button class="btn" onclick={upload}><Icon name="upload" size={20} /> <span class="lbl">{t('uploadToShare')}</span></button>
    {/if}
    {#if link?.allowDownload && assets.length}
      <button class="btn" onclick={() => downloadAssets(sorted, access)}><Icon name="download" size={20} /> <span class="lbl">{t('downloadAll')}</span></button>
    {/if}
    <button class="icon-btn" onclick={() => setTheme(ui.theme === 'dark' ? 'light' : 'dark')} aria-label={t('theme')}>
      <Icon name={ui.theme === 'dark' ? 'sun' : 'moon'} />
    </button>
  </header>

  <div class="page">
    {#if needPassword}
      <form class="card pw form" onsubmit={login}>
        <Icon name="lock" size={40} />
        <p>{t('sharePasswordRequired')}</p>
        <!-- svelte-ignore a11y_autofocus -->
        <input type="password" bind:value={password} autofocus aria-label={t('password')} />
        {#if error}<p class="error-text">{error}</p>{/if}
        <button class="btn primary" disabled={busy}>{t('unlock')}</button>
      </form>
    {:else if error}
      <div class="empty">{error}</div>
    {:else if !link}
      <div class="spinner"></div>
    {:else}
      <div class="head">
        <h1>{title}</h1>
        {#if link.album?.description}<p class="muted">{link.album.description}</p>{/if}
        <p class="muted small">{range}{range ? ' · ' : ''}{t('photosCount', { n: assets.length })}</p>
      </div>
      {#if !assets.length}
        <div class="empty">{t('albumEmpty')}</div>
      {:else}
        <AssetGrid
          assets={sorted}
          share={access}
          groupByDay={false}
          viewerOpts={{ allowDownload: link.allowDownload, showMetadata: link.showMetadata, readOnly: true }}
        />
      {/if}
    {/if}
  </div>
</div>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'u' && e.ctrlKey && link?.allowUpload) {
      e.preventDefault();
      upload();
      toast(t('uploadToShare'));
    }
  }}
/>

<style>
  .share {
    min-height: 100vh;
  }
  .share.dragging {
    outline: 4px dashed var(--primary);
    outline-offset: -8px;
  }
  .top {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 16px;
    height: 64px;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
  }
  .logo {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--fg);
    font-size: 1.2rem;
  }
  .spacer {
    flex: 1;
  }
  .head {
    margin: 16px 0 20px;
  }
  .head h1 {
    font-size: 2rem;
    font-weight: 400;
  }
  .pw {
    max-width: 380px;
    margin: 64px auto;
    align-items: center;
    text-align: center;
  }
  @media (max-width: 600px) {
    .logo :global(.wordmark),
    .lbl {
      display: none;
    }
    .top .btn {
      padding: 0 10px;
    }
  }
</style>
