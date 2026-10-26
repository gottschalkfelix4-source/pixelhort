<script lang="ts">
  import Icon from './Icon.svelte';
  import InfoPanel from './InfoPanel.svelte';
  import Editor from './Editor.svelte';
  import SphereViewer from './SphereViewer.svelte';
  import Pickers from './Pickers.svelte';
  import ShareLinkDialog from './ShareLinkDialog.svelte';
  import CastDialog from './CastDialog.svelte';
  import { api, originalUrl, thumbnailUrl, videoUrl, type AssetDto } from '../lib/api';
  import { auth, bumpAssets, closeViewer, confirmDialog, prefs, toast, toastError, ui, viewer } from '../lib/state.svelte';
  import { downloadAsset, formatLocal, isWebImage } from '../lib/utils';
  import { t } from '../lib/i18n';

  const ctx = $derived(viewer.ctx!);
  let index = $state(viewer.ctx?.index ?? 0);
  let detail = $state<AssetDto | null>(null);
  let showInfo = $state(typeof window !== 'undefined' && window.innerWidth > 1200 && localStorage.getItem('ph_info') === '1');
  let editing = $state(false);
  let picker = $state<'album' | null>(null);
  let sharing = $state(false);
  let menuOpen = $state(false);
  let dirty = false;
  let liveOn = $state(false);
  let cacheBust = $state<Record<string, string>>({});

  // zoom & pan
  let scale = $state(1);
  let tx = $state(0);
  let ty = $state(0);
  let hiRes = $state(false);

  const asset = $derived<AssetDto | undefined>(ctx?.assets[index]);
  const current = $derived(detail && asset && detail.id === asset.id ? detail : asset);
  const share = $derived(ctx?.share ?? null);
  const isOwner = $derived(!!current && !share && !ctx?.readOnly && current.ownerId === auth.user?.id);
  const canDownload = $derived(share ? ctx?.allowDownload !== false : true);
  const showMeta = $derived(share ? ctx?.showMetadata !== false : true);
  const is360 = $derived(current?.projectionType === 'EQUIRECTANGULAR' || current?.exif?.projectionType === 'EQUIRECTANGULAR');
  const isPano = $derived(!!current?.isPanorama && !is360);
  const bust = $derived(current ? cacheBust[current.id] : undefined);
  const previewSrc = $derived(current ? thumbnailUrl(current.id, 'preview', share, bust ?? (current.hasEdits ? current.updatedAt : undefined)) : '');
  const fullSrc = $derived(
    current && isWebImage(current.originalMimeType) && !current.isRaw
      ? current.hasEdits
        ? originalUrl(current.id, share, true)
        : originalUrl(current.id, share)
      : previewSrc,
  );

  // Load full detail whenever the asset changes.
  $effect(() => {
    const a = asset;
    if (!a) return;
    resetZoom();
    liveOn = false;
    hiRes = false;
    const id = a.id;
    api
      .asset(id, share)
      .then((d) => {
        if (asset?.id === id) detail = d;
      })
      .catch(() => {});
    // preload neighbours
    for (const n of [ctx.assets[index + 1], ctx.assets[index - 1]]) {
      if (n && n.type === 'IMAGE') new Image().src = thumbnailUrl(n.id, 'preview', share);
    }
  });

  $effect(() => {
    if (scale > 1.6 && !hiRes && current?.type === 'IMAGE') hiRes = true;
  });

  function resetZoom() {
    scale = 1;
    tx = 0;
    ty = 0;
  }

  function go(delta: number) {
    const n = index + delta;
    if (n < 0 || n >= ctx.assets.length) return;
    index = n;
  }

  function close() {
    if (dirty) {
      ctx.onChange?.();
    }
    closeViewer();
  }

  function replaceCurrent(a: AssetDto) {
    detail = a;
    const list = ctx.assets;
    const i = list.findIndex((x) => x.id === a.id);
    if (i >= 0) Object.assign(list[i], { ...a, exif: list[i].exif, tags: list[i].tags, people: list[i].people });
  }

  function removeCurrent() {
    dirty = true;
    bumpAssets();
    ctx.assets.splice(index, 1);
    if (!ctx.assets.length) return close();
    if (index >= ctx.assets.length) index = ctx.assets.length - 1;
    detail = null;
  }

  async function toggleFavorite() {
    if (!current || !isOwner) return;
    try {
      replaceCurrent(await api.updateAsset(current.id, { isFavorite: !current.isFavorite }));
      dirty = true;
    } catch (e) {
      toastError(e);
    }
  }

  async function setVisibility(v: 'timeline' | 'archive' | 'locked') {
    menuOpen = false;
    if (!current) return;
    try {
      await api.updateAsset(current.id, { visibility: v });
      toast(v === 'archive' ? t('archived') : v === 'locked' ? t('movedToLocked') : t('unarchived'), 'success');
      removeCurrent();
    } catch (e) {
      toastError(e);
    }
  }

  async function trash() {
    if (!current || !isOwner) return;
    try {
      if (current.isTrashed) {
        if (!(await confirmDialog(t('deletePermanently'), { message: t('deleteForeverConfirm', { n: 1 }), danger: true }))) return;
        await api.deleteAssets([current.id], true);
      } else {
        await api.deleteAssets([current.id]);
        toast(t('movedToTrash'));
      }
      removeCurrent();
    } catch (e) {
      toastError(e);
    }
  }

  async function restore() {
    if (!current) return;
    try {
      await api.restoreTrash([current.id]);
      toast(t('restore'), 'success');
      removeCurrent();
    } catch (e) {
      toastError(e);
    }
  }

  function download() {
    if (!current || !canDownload) return;
    downloadAsset(current, share, !share && prefs.value.download.includeLiveVideo);
  }

  let showCast = $state(false);

  function cast() {
    if (current) showCast = true;
  }

  function toggleInfo() {
    showInfo = !showInfo;
    try {
      localStorage.setItem('ph_info', showInfo ? '1' : '0');
    } catch {
      /* ignore */
    }
  }

  function onkey(e: KeyboardEvent) {
    if (editing || picker || sharing) return;
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA') return;
    switch (e.key) {
      case 'ArrowLeft':
        go(-1);
        break;
      case 'ArrowRight':
        go(1);
        break;
      case 'Escape':
        if (menuOpen) menuOpen = false;
        else close();
        break;
      case 'i':
        if (showMeta) toggleInfo();
        break;
      case 'f':
        toggleFavorite();
        break;
      case 'Delete':
        trash();
        break;
      case 'd':
        if (e.shiftKey) download();
        break;
      case 'e':
        if (isOwner && current?.type === 'IMAGE') editing = true;
        break;
      case '+':
      case '=':
        scale = Math.min(8, scale * 1.5);
        break;
      case '-':
        scale = Math.max(1, scale / 1.5);
        if (scale === 1) resetZoom();
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  // ── zoom / pan / swipe ──
  const pointers = new Map<number, { x: number; y: number }>();
  let pinchStart: { dist: number; scale: number } | null = null;
  let panStart: { x: number; y: number; tx: number; ty: number } | null = null;
  let swipeStart: { x: number; y: number; t: number } | null = null;

  function onWheel(e: WheelEvent) {
    if (current?.type !== 'IMAGE' || is360 || isPano) return;
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    const ns = Math.min(8, Math.max(1, scale * factor));
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const cx = e.clientX - rect.left - rect.width / 2;
    const cy = e.clientY - rect.top - rect.height / 2;
    tx = cx - ((cx - tx) * ns) / scale;
    ty = cy - ((cy - ty) * ns) / scale;
    scale = ns;
    if (scale === 1) resetZoom();
  }

  function onDblClick(e: MouseEvent) {
    if (current?.type !== 'IMAGE' || is360 || isPano) return;
    if (scale > 1) return resetZoom();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const cx = e.clientX - rect.left - rect.width / 2;
    const cy = e.clientY - rect.top - rect.height / 2;
    scale = 2.5;
    tx = -cx * 1.5;
    ty = -cy * 1.5;
  }

  function onPointerDown(e: PointerEvent) {
    if (current?.type !== 'IMAGE' || is360 || isPano) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      pinchStart = { dist: Math.hypot(a.x - b.x, a.y - b.y), scale };
      swipeStart = null;
    } else if (pointers.size === 1) {
      panStart = { x: e.clientX, y: e.clientY, tx, ty };
      swipeStart = scale === 1 ? { x: e.clientX, y: e.clientY, t: Date.now() } : null;
    }
  }
  function onPointerMove(e: PointerEvent) {
    if (!pointers.has(e.pointerId)) return;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.size === 2 && pinchStart) {
      const [a, b] = [...pointers.values()];
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      scale = Math.min(8, Math.max(1, (pinchStart.scale * d) / pinchStart.dist));
    } else if (pointers.size === 1 && panStart && scale > 1) {
      tx = panStart.tx + (e.clientX - panStart.x);
      ty = panStart.ty + (e.clientY - panStart.y);
    }
  }
  function onPointerUp(e: PointerEvent) {
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinchStart = null;
    if (swipeStart && pointers.size === 0) {
      const dx = e.clientX - swipeStart.x;
      const dy = e.clientY - swipeStart.y;
      if (Date.now() - swipeStart.t < 600 && Math.abs(dx) > 60 && Math.abs(dx) > Math.abs(dy) * 1.5) go(dx < 0 ? 1 : -1);
      else if (Date.now() - swipeStart.t < 600 && dy > 120 && Math.abs(dy) > Math.abs(dx) * 1.5) close();
    }
    if (scale <= 1) resetZoom();
    if (!pointers.size) {
      panStart = null;
      swipeStart = null;
    }
  }
</script>

<svelte:window onkeydown={onkey} />

{#if ctx && current}
  <div class="viewer">
    <div class="stage-col">
      <header class="bar">
        <button class="icon-btn" onclick={close} aria-label={t('back')}><Icon name="back" /></button>
        <div class="title">
          <span class="date">{formatLocal(current.localDateTime, { dateStyle: 'medium', timeStyle: 'short' })}</span>
          <span class="count">{index + 1} / {ctx.assets.length}</span>
        </div>
        <span class="spacer"></span>
        {#if current.livePhotoVideoId && current.type === 'IMAGE'}
          <button
            class="live-btn"
            class:on={liveOn}
            onmouseenter={() => (liveOn = true)}
            onmouseleave={() => (liveOn = false)}
            onpointerdown={() => (liveOn = true)}
            onpointerup={(e) => e.pointerType !== 'mouse' && (liveOn = false)}
            onpointercancel={() => (liveOn = false)}
          >
            <Icon name="live" size={18} />
            {t('live')}
          </button>
        {/if}
        {#if auth.serverInfo?.features.cast !== false && !share}
          <button class="icon-btn" onclick={cast} title={t('cast')} aria-label={t('cast')}><Icon name="cast" /></button>
        {/if}
        {#if isOwner && !current.isTrashed}
          <button class="icon-btn" onclick={() => (sharing = true)} title={t('share')} aria-label={t('share')}><Icon name="share" /></button>
          {#if current.type === 'IMAGE'}
            <button class="icon-btn" onclick={() => (editing = true)} title={t('editImage')} aria-label={t('editImage')}><Icon name="tune" /></button>
          {/if}
          <button class="icon-btn" onclick={toggleFavorite} title={t('favorite')} aria-label={t('favorite')}>
            <Icon name={current.isFavorite ? 'heart' : 'heartOutline'} />
          </button>
        {/if}
        {#if showMeta}
          <button class="icon-btn" class:active={showInfo} onclick={toggleInfo} title={t('info')} aria-label={t('info')}><Icon name="info" /></button>
        {/if}
        {#if canDownload}
          <button class="icon-btn" onclick={download} title={t('download')} aria-label={t('download')}><Icon name="download" /></button>
        {/if}
        {#if isOwner}
          {#if current.isTrashed}
            <button class="icon-btn" onclick={restore} title={t('restore')} aria-label={t('restore')}><Icon name="restore" /></button>
          {/if}
          <button class="icon-btn" onclick={trash} title={current.isTrashed ? t('deletePermanently') : t('moveToTrash')} aria-label={t('delete')}>
            <Icon name="delete" />
          </button>
          {#if !current.isTrashed}
            <div class="menu-wrap">
              <button class="icon-btn" onclick={() => (menuOpen = !menuOpen)} aria-label={t('more')}><Icon name="moreVert" /></button>
              {#if menuOpen}
                <div class="menu">
                  <button
                    onclick={() => {
                      menuOpen = false;
                      picker = 'album';
                    }}><Icon name="album" size={20} /> {t('addToAlbum')}</button
                  >
                  {#if current.visibility === 'archive'}
                    <button onclick={() => setVisibility('timeline')}><Icon name="unarchive" size={20} /> {t('unarchive')}</button>
                  {:else if current.visibility !== 'locked'}
                    <button onclick={() => setVisibility('archive')}><Icon name="archive" size={20} /> {t('archive')}</button>
                  {/if}
                  {#if current.visibility === 'locked'}
                    <button onclick={() => setVisibility('timeline')}><Icon name="lockOpen" size={20} /> {t('moveOutOfLocked')}</button>
                  {:else}
                    <button onclick={() => setVisibility('locked')}><Icon name="lock" size={20} /> {t('moveToLocked')}</button>
                  {/if}
                </div>
              {/if}
            </div>
          {/if}
        {/if}
      </header>

      <div
        class="stage"
        role="presentation"
        onwheel={onWheel}
        ondblclick={onDblClick}
        onpointerdown={onPointerDown}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        onpointercancel={onPointerUp}
      >
        {#key current.id}
          {#if current.type === 'VIDEO'}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video
              class="media"
              src={videoUrl(current.id, share)}
              poster={previewSrc}
              controls
              autoplay={ui.videoAutoplay}
              loop={ui.videoLoop}
              muted={ui.videoMuted}
              playsinline
            ></video>
          {:else if is360}
            <SphereViewer src={fullSrc} />
          {:else if isPano}
            <div class="pano">
              <img src={fullSrc} alt={current.originalFileName} draggable="false" />
            </div>
          {:else}
            <div class="zoom" style="transform: translate({tx}px, {ty}px) scale({scale})">
              <img class="media" src={hiRes ? fullSrc : previewSrc} alt={current.originalFileName} draggable="false" />
              {#if liveOn && current.livePhotoVideoId}
                <video
                  class="media live"
                  src={videoUrl(current.livePhotoVideoId, share)}
                  autoplay
                  muted
                  playsinline
                  onended={() => (liveOn = false)}
                ></video>
              {/if}
            </div>
          {/if}
        {/key}
        {#if index > 0}
          <button class="nav prev" onclick={() => go(-1)} aria-label={t('previous')}><Icon name="chevronLeft" size={32} /></button>
        {/if}
        {#if index < ctx.assets.length - 1}
          <button class="nav next" onclick={() => go(1)} aria-label={t('next')}><Icon name="chevronRight" size={32} /></button>
        {/if}
        <div class="badges">
          {#if current.isRaw}<span class="badge">{t('rawFile')}</span>{/if}
          {#if is360}<span class="badge">{t('sphere')}</span>{/if}
          {#if isPano}<span class="badge">{t('panorama')}</span>{/if}
          {#if current.isOffline}<span class="badge">{t('offline')}</span>{/if}
        </div>
      </div>
    </div>

    {#if showInfo && showMeta}
      <InfoPanel asset={current} {share} readOnly={!isOwner} onupdate={replaceCurrent} onclose={toggleInfo} />
    {/if}

    {#if editing}
      <Editor
        asset={current}
        onclose={() => (editing = false)}
        onsaved={(a, copy) => {
          editing = false;
          dirty = true;
          bumpAssets();
          if (copy) {
            ctx.assets.splice(index, 0, a);
          } else {
            cacheBust = { ...cacheBust, [a.id]: String(Date.now()) };
            replaceCurrent(a);
          }
        }}
      />
    {/if}
  </div>

  {#if picker === 'album'}
    <Pickers
      kind="album"
      onclose={() => (picker = null)}
      onpick={async (p) => {
        picker = null;
        if (p.kind !== 'album') return;
        try {
          await api.addAlbumAssets(p.album.id, [current!.id]);
          toast(t('addedToAlbum', { n: 1, album: p.album.albumName }), 'success');
        } catch (e) {
          toastError(e);
        }
      }}
    />
  {/if}
  {#if sharing}
    <ShareLinkDialog assetIds={[current.id]} onclose={() => (sharing = false)} />
  {/if}
  {#if showCast}
    <CastDialog asset={current} onclose={() => (showCast = false)} />
  {/if}
{/if}

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 500;
    background: #000;
    color: #fff;
    display: flex;
  }
  .stage-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    position: relative;
  }
  .bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 8px;
    background: linear-gradient(rgba(0, 0, 0, 0.6), transparent);
    color: #fff;
  }
  .bar .icon-btn:hover {
    background: rgba(255, 255, 255, 0.15);
  }
  .bar .icon-btn.active {
    color: #8ab4f8;
  }
  .title {
    display: flex;
    flex-direction: column;
    margin-left: 4px;
    line-height: 1.2;
    min-width: 0;
  }
  .date {
    font-size: 0.95rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count {
    font-size: 0.75rem;
    opacity: 0.7;
  }
  .spacer {
    flex: 1;
  }
  .live-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 1px solid rgba(255, 255, 255, 0.6);
    background: rgba(0, 0, 0, 0.3);
    color: #fff;
    border-radius: 14px;
    padding: 3px 10px 3px 6px;
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    cursor: pointer;
    margin-right: 4px;
  }
  .live-btn.on {
    background: #fff;
    color: #000;
  }
  .stage {
    flex: 1;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    touch-action: none;
    user-select: none;
  }
  .zoom {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    transform-origin: center;
    will-change: transform;
  }
  .media {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  video.media {
    width: 100%;
    height: 100%;
    touch-action: auto;
  }
  .live {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 100%;
    height: 100%;
  }
  .pano {
    width: 100%;
    height: 100%;
    overflow-x: auto;
    overflow-y: hidden;
    touch-action: pan-x;
  }
  .pano img {
    height: 100%;
    width: auto;
    max-width: none;
    display: block;
  }
  .nav {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 48px;
    height: 48px;
    border-radius: 50%;
    border: none;
    background: rgba(0, 0, 0, 0.35);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s;
    z-index: 4;
  }
  .stage:hover .nav {
    opacity: 1;
  }
  .nav.prev {
    left: 16px;
  }
  .nav.next {
    right: 16px;
  }
  .badges {
    position: absolute;
    bottom: 12px;
    left: 12px;
    display: flex;
    gap: 6px;
    z-index: 4;
  }
  .badges .badge {
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
  }
  .menu-wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 44px;
    background: #2d2e30;
    border-radius: 8px;
    padding: 6px 0;
    min-width: 230px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    z-index: 10;
  }
  .menu button {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 10px 16px;
    border: none;
    background: none;
    color: #eee;
    cursor: pointer;
    text-align: left;
  }
  .menu button:hover {
    background: rgba(255, 255, 255, 0.08);
  }
  @media (hover: none) {
    .nav {
      display: none;
    }
  }
  @media (max-width: 700px) {
    .viewer {
      flex-direction: column;
    }
    .viewer :global(.info) {
      position: absolute;
      inset: auto 0 0 0;
      height: 60%;
      z-index: 8;
      border-radius: 16px 16px 0 0;
    }
  }
</style>
