<script lang="ts">
  import Modal from './Modal.svelte';
  import { api, personThumbnailUrl, type MetadataSearch, type PersonDto, type SuggestionType, type TagDto } from '../lib/api';
  import { debounce } from '../lib/utils';
  import { t } from '../lib/i18n';

  let {
    initial = {},
    onapply,
    onclose,
  }: { initial?: MetadataSearch; onapply: (f: MetadataSearch) => void; onclose: () => void } = $props();

  // svelte-ignore state_referenced_locally
  const init = initial;
  let f = $state<MetadataSearch>({ ...init });
  let objectsText = $state((init.objects ?? []).join(', '));
  let people = $state<PersonDto[]>([]);
  let tags = $state<TagDto[]>([]);
  let sugg = $state<Record<SuggestionType, string[]>>({ city: [], country: [], make: [], model: [], object: [] });

  $effect(() => {
    api
      .people()
      .then((r) => (people = r.people.filter((p) => p.name).slice(0, 60)))
      .catch(() => {});
    api
      .tags()
      .then((r) => (tags = r))
      .catch(() => {});
    for (const k of ['city', 'country', 'make', 'model', 'object'] as SuggestionType[]) loadSugg(k, '');
  });

  function loadSugg(type: SuggestionType, q: string) {
    api
      .suggestions(type, q)
      .then((r) => (sugg[type] = r.slice(0, 50)))
      .catch(() => {});
  }
  const suggest = debounce(loadSugg, 250);

  function togglePerson(id: string) {
    const list = f.personIds ?? [];
    f.personIds = list.includes(id) ? list.filter((x) => x !== id) : [...list, id];
  }
  function toggleTag(id: string) {
    const list = f.tagIds ?? [];
    f.tagIds = list.includes(id) ? list.filter((x) => x !== id) : [...list, id];
  }

  function dateVal(iso?: string) {
    return iso ? iso.slice(0, 10) : '';
  }

  function apply(e: Event) {
    e.preventDefault();
    const out: MetadataSearch = {};
    const objects = objectsText
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
    for (const [k, v] of Object.entries($state.snapshot(f))) {
      if (v === '' || v === undefined || v === null || v === false) continue;
      if (Array.isArray(v) && !v.length) continue;
      (out as Record<string, unknown>)[k] = v;
    }
    if (objects.length) out.objects = objects;
    else delete out.objects;
    delete out.page;
    onapply(out);
  }

  function clear() {
    f = {};
    objectsText = '';
  }
</script>

<Modal title={t('searchFilters')} {onclose} width={640}>
  <form id="search-filter" class="form" onsubmit={apply}>
    <div class="grid-2">
      <label class="field">{t('fileName')}<input type="text" bind:value={f.originalFileName} /></label>
      <label class="field">{t('ocrText')}<input type="text" bind:value={f.text} /></label>
      <label class="field"
        >{t('make')}<input type="text" list="dl-make" bind:value={f.make} oninput={() => suggest('make', f.make ?? '')} /></label
      >
      <label class="field"
        >{t('model')}<input type="text" list="dl-model" bind:value={f.model} oninput={() => suggest('model', f.model ?? '')} /></label
      >
      <label class="field">{t('lens')}<input type="text" bind:value={f.lensModel} /></label>
      <label class="field"
        >{t('city')}<input type="text" list="dl-city" bind:value={f.city} oninput={() => suggest('city', f.city ?? '')} /></label
      >
      <label class="field">{t('state')}<input type="text" bind:value={f.state} /></label>
      <label class="field"
        >{t('country')}<input type="text" list="dl-country" bind:value={f.country} oninput={() => suggest('country', f.country ?? '')} /></label
      >
      <label class="field"
        >{t('takenAfter')}<input
          type="date"
          value={dateVal(f.takenAfter)}
          onchange={(e) => {
            const v = (e.currentTarget as HTMLInputElement).value;
            f.takenAfter = v ? `${v}T00:00:00Z` : undefined;
          }}
        /></label
      >
      <label class="field"
        >{t('takenBefore')}<input
          type="date"
          value={dateVal(f.takenBefore)}
          onchange={(e) => {
            const v = (e.currentTarget as HTMLInputElement).value;
            f.takenBefore = v ? `${v}T23:59:59Z` : undefined;
          }}
        /></label
      >
      <label class="field"
        >{t('mediaType')}
        <select bind:value={f.type}>
          <option value={undefined}>{t('typeAll')}</option>
          <option value="IMAGE">{t('typeImage')}</option>
          <option value="VIDEO">{t('typeVideo')}</option>
        </select>
      </label>
      <label class="field"
        >{t('objects')}<input
          type="text"
          list="dl-object"
          bind:value={objectsText}
          placeholder="dog, beach"
          oninput={() => suggest('object', objectsText.split(',').pop()?.trim() ?? '')}
        /></label
      >
    </div>

    <div class="row wrap">
      <label class="check"><input type="checkbox" bind:checked={f.isFavorite} /> {t('onlyFavorites')}</label>
      <label class="check"><input type="checkbox" bind:checked={f.isRaw} /> {t('onlyRaw')}</label>
      <label class="check"><input type="checkbox" bind:checked={f.isPanorama} /> {t('onlyPanorama')}</label>
      <label class="check"><input type="checkbox" bind:checked={f.is360} /> {t('only360')}</label>
      <label class="check"><input type="checkbox" bind:checked={f.isMotion} /> {t('onlyMotion')}</label>
    </div>

    {#if people.length}
      <div>
        <h3 class="sub">{t('people')}</h3>
        <div class="people">
          {#each people as p (p.id)}
            <button type="button" class="person" class:active={f.personIds?.includes(p.id)} onclick={() => togglePerson(p.id)}>
              <img src={personThumbnailUrl(p.id)} alt="" />
              <span>{p.name}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
    {#if tags.length}
      <div>
        <h3 class="sub">{t('tags')}</h3>
        <div class="row wrap">
          {#each tags as x (x.id)}
            <button type="button" class="chip" class:active={f.tagIds?.includes(x.id)} onclick={() => toggleTag(x.id)}>{x.value}</button>
          {/each}
        </div>
      </div>
    {/if}

    {#each Object.entries(sugg) as [k, list] (k)}
      <datalist id="dl-{k}">
        {#each list as v (v)}<option value={v}></option>{/each}
      </datalist>
    {/each}
  </form>
  {#snippet footer()}
    <button class="btn text" onclick={clear}>{t('reset')}</button>
    <span class="spacer"></span>
    <button class="btn text" onclick={onclose}>{t('cancel')}</button>
    <button class="btn primary" type="submit" form="search-filter">{t('search')}</button>
  {/snippet}
</Modal>

<style>
  .sub {
    font-size: 0.85rem;
    color: var(--fg-muted);
    margin: 4px 0 8px;
  }
  .people {
    display: flex;
    gap: 10px;
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .person {
    border: none;
    background: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 64px;
    cursor: pointer;
    font-size: 0.75rem;
    flex-shrink: 0;
  }
  .person img {
    width: 52px;
    height: 52px;
    border-radius: 50%;
    object-fit: cover;
    border: 3px solid transparent;
  }
  .person.active img {
    border-color: var(--primary);
  }
  .person span {
    max-width: 64px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
