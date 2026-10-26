<script lang="ts">
  import Icon from './Icon.svelte';
  import { api, thumbnailUrl, videoUrl, type Memory } from '../lib/api';
  import { openViewer } from '../lib/state.svelte';
  import { thumbhashUrl } from '../lib/utils';
  import { t } from '../lib/i18n';

  let memories = $state<Memory[]>([]);
  let open = $state<number | null>(null);
  let item = $state(0);
  let paused = $state(false);
  let progress = $state(0);
  const DURATION = 5000;

  $effect(() => {
    api
      .memories()
      .then((m) => (memories = m.filter((x) => x.assets.length)))
      .catch(() => {});
  });

  function title(m: Memory) {
    if (m.yearsAgo === 1) return t('yearAgo');
    if (m.yearsAgo > 1) return t('yearsAgo', { n: m.yearsAgo });
    return m.title;
  }

  const mem = $derived(open !== null ? memories[open] : null);
  const cur = $derived(mem ? mem.assets[item] : null);

  function start(i: number) {
    open = i;
    item = 0;
    progress = 0;
    paused = false;
  }
  function close() {
    open = null;
  }
  function next() {
    if (!mem) return;
    progress = 0;
    if (item < mem.assets.length - 1) item++;
    else if (open! < memories.length - 1) start(open! + 1);
    else close();
  }
  function prev() {
    progress = 0;
    if (item > 0) item--;
    else if (open! > 0) {
      const p = open! - 1;
      start(p);
      item = memories[p].assets.length - 1;
    }
  }

  // Timer for images (videos advance on `ended`).
  $effect(() => {
    if (open === null || !cur || cur.type === 'VIDEO' || paused) return;
    let last = performance.now();
    let raf = 0;
    const tick = (now: number) => {
      progress += (now - last) / DURATION;
      last = now;
      if (progress >= 1) next();
      else raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });

  function onkey(e: KeyboardEvent) {
    if (open === null) return;
    if (e.key === 'Escape') close();
    else if (e.key === 'ArrowRight') next();
    else if (e.key === 'ArrowLeft') prev();
    else if (e.key === ' ') paused = !paused;
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  let longPress = false;
  function down() {
    longPress = false;
    pressTimer = setTimeout(() => {
      longPress = true;
      paused = true;
    }, 250);
  }
  function up(e: PointerEvent) {
    clearTimeout(pressTimer);
    if (longPress) {
      paused = false;
      return;
    }
    const x = e.clientX / window.innerWidth;
    if (x < 0.3) prev();
    else next();
  }
</script>

<svelte:window onkeydown={onkey} />

{#if memories.length}
  <div class="bar" aria-label={t('memories')}>
    {#each memories as m, i (m.id)}
      {@const cover = m.assets[0]}
      <button class="card" onclick={() => start(i)} style={cover.thumbhash ? `background-image:url(${thumbhashUrl(cover.thumbhash)})` : ''}>
        <img src={thumbnailUrl(cover.id, 'preview')} alt="" loading="lazy" />
        <span class="label">{title(m)}</span>
      </button>
    {/each}
  </div>
{/if}

{#if mem && cur}
  <div class="story" role="dialog" aria-modal="true" aria-label={title(mem)}>
    <div class="progress">
      {#each mem.assets as a, i (a.id)}
        <div class="seg"><div style="width: {i < item ? 100 : i === item ? progress * 100 : 0}%"></div></div>
      {/each}
    </div>
    <header>
      <div>
        <strong>{title(mem)}</strong>
        <small>{mem.year}</small>
      </div>
      <span class="spacer"></span>
      <button class="icon-btn" onclick={() => (paused = !paused)} aria-label={paused ? 'Play' : 'Pause'}>
        <Icon name={paused ? 'play' : 'pause'} />
      </button>
      <button
        class="icon-btn"
        onclick={() => {
          const list = mem.assets;
          const idx = item;
          close();
          openViewer({ assets: list, index: idx });
        }}
        aria-label={t('info')}><Icon name="openNew" /></button
      >
      <button class="icon-btn" onclick={close} aria-label={t('close')}><Icon name="close" /></button>
    </header>
    <div class="media" role="presentation" onpointerdown={down} onpointerup={up}>
      {#key cur.id}
        {#if cur.type === 'VIDEO'}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video
            src={videoUrl(cur.id)}
            autoplay
            playsinline
            muted
            ontimeupdate={(e) => {
              const v = e.currentTarget as HTMLVideoElement;
              if (v.duration) progress = v.currentTime / v.duration;
            }}
            onended={next}
          ></video>
        {:else}
          <img class="kenburns" class:paused src={thumbnailUrl(cur.id, 'preview')} alt="" draggable="false" />
        {/if}
      {/key}
    </div>
  </div>
{/if}

<style>
  .bar {
    display: flex;
    gap: 12px;
    overflow-x: auto;
    padding: 12px 0 8px;
    scrollbar-width: none;
  }
  .card {
    position: relative;
    flex: 0 0 auto;
    width: 150px;
    height: 210px;
    border-radius: 16px;
    box-shadow: var(--shadow-sm);
    overflow: hidden;
    border: none;
    padding: 0;
    cursor: pointer;
    background: var(--placeholder) center/cover;
  }
  .card img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: transform 0.3s;
  }
  .card:hover img {
    transform: scale(1.05);
  }
  .label {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 24px 12px 12px;
    color: #fff;
    font-weight: 700;
    line-height: 1.25;
    text-align: left;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.75));
  }
  .story {
    position: fixed;
    inset: 0;
    background: #000;
    z-index: 600;
    color: #fff;
    display: flex;
    flex-direction: column;
  }
  .progress {
    display: flex;
    gap: 4px;
    padding: 10px 12px 0;
  }
  .seg {
    flex: 1;
    height: 3px;
    background: rgba(255, 255, 255, 0.3);
    border-radius: 2px;
    overflow: hidden;
  }
  .seg div {
    height: 100%;
    background: #fff;
  }
  header {
    display: flex;
    align-items: center;
    padding: 8px 12px;
    gap: 4px;
  }
  header div {
    display: flex;
    flex-direction: column;
  }
  header small {
    opacity: 0.7;
  }
  .spacer {
    flex: 1;
  }
  .icon-btn:hover {
    background: rgba(255, 255, 255, 0.15);
  }
  .media {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    user-select: none;
  }
  .media img,
  .media video {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .kenburns {
    animation: kb 5s ease-out forwards;
  }
  .kenburns.paused {
    animation-play-state: paused;
  }
  @keyframes kb {
    from {
      transform: scale(1);
    }
    to {
      transform: scale(1.08);
    }
  }
  @media (max-width: 600px) {
    .card {
      width: 116px;
      height: 164px;
    }
    .label {
      font-size: 0.85rem;
      padding: 24px 10px 10px;
    }
  }
</style>
