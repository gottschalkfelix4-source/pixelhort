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
  <div class="page-header"><h1>{t('navExplore')}</h1></div>
  <form class="big-search" onsubmit={submit} role="search">
    <Icon name="search" />
    <input type="search" bind:value={q} placeholder={t('smartSearchHint')} />
    <a class="icon-btn" href="/search?filter=1" aria-label={t('searchFilters')}><Icon name="tune" /></a>
  </form>
  <div class="row wrap quick">
    <button class="chip" onclick={() => metadata({ type: 'VIDEO' })}><Icon name="video" size={18} /> {t('videos')}</button>
    <button class="chip" onclick={() => metadata({ isFavorite: true })}><Icon name="heartOutline" size={18} /> {t('navFavorites')}</button>
    <button class="chip" onclick={() => metadata({ isMotion: true })}><Icon name="live" size={18} /> {t('onlyMotion')}</button>
    <button class="chip" onclick={() => metadata({ is360: true })}><Icon name="sphere" size={18} /> {t('only360')}</button>
    <button class="chip" onclick={() => metadata({ isPanorama: true })}><Icon name="panorama" size={18} /> {t('onlyPanorama')}</button>
    <button class="chip" onclick={() => metadata({ isRaw: true })}><Icon name="camera" size={18} /> RAW</button>
  </div>

  {#if !data}
    <div class="spinner"></div>
  {:else}
    {#if data.people.length}
      <section>
        <div class="section-title"><h2>{t('people')}</h2><span class="spacer"></span><a href="/people">{t('all')}</a></div>
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
        <div class="section-title"><h2>{t('places')}</h2><span class="spacer"></span><a href="/map">{t('navMap')}</a></div>
        <div class="tiles">
          {#each data.places as p (p.value)}
            <button class="tile" onclick={() => metadata({ city: p.value })}>
              <span class="fallback"><Icon name="place" size={32} /></span>
              <img src={thumbnailUrl(p.assetId)} alt="" loading="lazy" onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = 'none')} />
              <span class="label">{p.value}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
    {#if data.objects.length}
      <section>
        <div class="section-title"><h2>{t('things')}</h2></div>
        <div class="tiles">
          {#each data.objects as o (o.value)}
            <button class="tile" onclick={() => metadata({ objects: [o.value] })}>
              <span class="fallback"><Icon name="photo" size={32} /></span>
              <img src={thumbnailUrl(o.assetId)} alt="" loading="lazy" onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = 'none')} />
              <span class="label">{o.value}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  .big-search {
    display: flex;
    align-items: center;
    gap: 12px;
    background: var(--bg-soft);
    border: 1px solid transparent;
    border-radius: 999px;
    padding: 0 8px 0 20px;
    height: 56px;
    margin: 4px 0 14px;
    color: var(--fg-muted);
    max-width: 760px;
    transition:
      background 0.15s,
      box-shadow 0.15s,
      border-color 0.15s;
  }
  .big-search:focus-within {
    background: var(--bg-elev);
    border-color: var(--border);
    box-shadow: var(--shadow);
  }
  .big-search input,
  .big-search input:focus {
    border: none;
    background: transparent;
    font-size: 1.05rem;
    box-shadow: none;
    padding: 0;
  }
  .quick {
    gap: 8px;
    margin-bottom: 36px;
  }
  section {
    margin-bottom: 40px;
  }
  .people {
    display: flex;
    gap: 20px;
    overflow-x: auto;
    padding: 4px 2px 8px;
  }
  .person {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 112px;
    flex-shrink: 0;
    color: var(--fg);
    font-size: 0.88rem;
    font-weight: 600;
    text-align: center;
  }
  .person span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .person img {
    width: 104px;
    height: 104px;
    border-radius: 50%;
    object-fit: cover;
    background: var(--placeholder);
    box-shadow: var(--shadow-sm);
    transition: transform 0.2s ease;
  }
  .person:hover img {
    transform: scale(1.05);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 12px;
  }
  .tile {
    position: relative;
    aspect-ratio: 1;
    border: none;
    padding: 0;
    border-radius: var(--radius);
    overflow: hidden;
    cursor: pointer;
    background: var(--placeholder);
  }
  .fallback {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-muted);
    opacity: 0.5;
  }
  .tile img {
    position: relative;
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: transform 0.35s ease;
  }
  .tile:hover img {
    transform: scale(1.06);
  }
  .label {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 32px 12px 10px;
    color: #fff;
    font-weight: 600;
    text-align: left;
    text-transform: capitalize;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.75));
  }
  @media (max-width: 600px) {
    .tiles {
      grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
      gap: 8px;
    }
    .big-search {
      height: 50px;
    }
  }
</style>
