<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import { api, personThumbnailUrl, thumbnailUrl, type ExploreResponse } from '../lib/api';
  import { navigate } from '../lib/router.svelte';
  import { toastError } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  let data = $state<ExploreResponse | null>(null);
  let q = $state('');

  $effect(() => {
    api
      .explore()
      .then((d) => (data = d))
      .catch(toastError);
  });

  function metadata(f: Record<string, unknown>) {
    navigate(`/search?f=${encodeURIComponent(JSON.stringify(f))}`);
  }

  function submit(e: Event) {
    e.preventDefault();
    if (q.trim()) navigate(`/search?q=${encodeURIComponent(q.trim())}`);
  }
</script>

<div class="page">
  <form class="big-search" onsubmit={submit} role="search">
    <Icon name="search" />
    <input type="search" bind:value={q} placeholder={t('smartSearchHint')} />
    <a class="icon-btn" href="/search?filter=1" aria-label={t('searchFilters')}><Icon name="tune" /></a>
  </form>

  {#if !data}
    <div class="spinner"></div>
  {:else}
    {#if data.people.length}
      <section>
        <div class="row head"><h2>{t('people')}</h2><span class="spacer"></span><a href="/people">{t('all')}</a></div>
        <div class="people">
          {#each data.people as p (p.id)}
            <a class="person" href="/people/{p.id}">
              <img src={personThumbnailUrl(p.id)} alt="" loading="lazy" />
              <span>{p.name || t('unnamedPerson')}</span>
            </a>
          {/each}
        </div>
      </section>
    {/if}
    {#if data.places.length}
      <section>
        <div class="row head"><h2>{t('places')}</h2><span class="spacer"></span><a href="/map">{t('navMap')}</a></div>
        <div class="tiles">
          {#each data.places as p (p.value)}
            <button class="tile" onclick={() => metadata({ city: p.value })}>
              <img src={thumbnailUrl(p.assetId)} alt="" loading="lazy" />
              <span>{p.value}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
    {#if data.objects.length}
      <section>
        <div class="row head"><h2>{t('things')}</h2></div>
        <div class="tiles">
          {#each data.objects as o (o.value)}
            <button class="tile" onclick={() => metadata({ objects: [o.value] })}>
              <img src={thumbnailUrl(o.assetId)} alt="" loading="lazy" />
              <span>{o.value}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
    <section>
      <div class="row wrap">
        <button class="chip" onclick={() => metadata({ type: 'VIDEO' })}><Icon name="video" size={18} /> {t('videos')}</button>
        <button class="chip" onclick={() => metadata({ isFavorite: true })}><Icon name="heart" size={18} /> {t('navFavorites')}</button>
        <button class="chip" onclick={() => metadata({ isMotion: true })}><Icon name="live" size={18} /> {t('onlyMotion')}</button>
        <button class="chip" onclick={() => metadata({ is360: true })}><Icon name="sphere" size={18} /> {t('only360')}</button>
        <button class="chip" onclick={() => metadata({ isPanorama: true })}><Icon name="panorama" size={18} /> {t('onlyPanorama')}</button>
        <button class="chip" onclick={() => metadata({ isRaw: true })}>RAW</button>
      </div>
    </section>
  {/if}
</div>

<style>
  .big-search {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--bg-soft);
    border-radius: 28px;
    padding: 0 8px 0 18px;
    height: 54px;
    margin: 8px 0 24px;
    color: var(--fg-muted);
    max-width: 800px;
  }
  .big-search input {
    border: none;
    background: transparent;
    font-size: 1.05rem;
  }
  .big-search input:focus {
    outline: none;
  }
  section {
    margin-bottom: 28px;
  }
  .head {
    margin-bottom: 12px;
  }
  .people {
    display: flex;
    gap: 16px;
    overflow-x: auto;
    padding-bottom: 6px;
  }
  .person {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 96px;
    flex-shrink: 0;
    color: var(--fg);
    font-size: 0.85rem;
    text-align: center;
  }
  .person img {
    width: 88px;
    height: 88px;
    border-radius: 50%;
    object-fit: cover;
    background: var(--placeholder);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 10px;
  }
  .tile {
    position: relative;
    aspect-ratio: 1;
    border: none;
    padding: 0;
    border-radius: 12px;
    overflow: hidden;
    cursor: pointer;
    background: var(--placeholder);
  }
  .tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .tile span {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 20px 10px 8px;
    color: #fff;
    font-weight: 500;
    text-align: left;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.7));
  }
  @media (max-width: 600px) {
    .tiles {
      grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    }
  }
</style>
