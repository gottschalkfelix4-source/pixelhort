<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import { api, personThumbnailUrl, type PersonDto } from '../lib/api';
  import { confirmDialog, toastError } from '../lib/state.svelte';
  import { t } from '../lib/i18n';
  import { SvelteSet } from 'svelte/reactivity';

  let people = $state<PersonDto[]>([]);
  let total = $state(0);
  let hidden = $state(0);
  let withHidden = $state(false);
  let loading = $state(true);
  let mergeMode = $state(false);
  const mergeSel = new SvelteSet<string>();

  async function load(h: boolean) {
    loading = true;
    try {
      const r = await api.people(h);
      people = r.people;
      total = r.total;
      hidden = r.hidden;
    } catch (e) {
      toastError(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    load(withHidden);
  });

  async function rename(p: PersonDto, name: string) {
    if (name === p.name) return;
    try {
      const r = await api.updatePerson(p.id, { name });
      p.name = r.name;
    } catch (e) {
      toastError(e);
    }
  }

  async function toggleHidden(p: PersonDto) {
    try {
      const r = await api.updatePerson(p.id, { isHidden: !p.isHidden });
      p.isHidden = r.isHidden;
      if (!withHidden && r.isHidden) people = people.filter((x) => x.id !== p.id);
    } catch (e) {
      toastError(e);
    }
  }

  async function doMerge() {
    const ids = [...mergeSel];
    if (ids.length < 2) return;
    // keep the person with the most faces (named preferred)
    const sel = people.filter((p) => mergeSel.has(p.id)).sort((a, b) => Number(!!b.name) - Number(!!a.name) || b.faceCount - a.faceCount);
    const target = sel[0];
    if (!(await confirmDialog(t('merge'), { message: t('mergeConfirm', { n: ids.length }) + ` → ${target.name || t('unnamedPerson')}` })))
      return;
    try {
      await api.mergePeople(
        target.id,
        ids.filter((x) => x !== target.id),
      );
      mergeSel.clear();
      mergeMode = false;
      load(withHidden);
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{t('navPeople')}</h1>
    <span class="muted small">{t('peopleCount', { n: total })}</span>
    {#if hidden || withHidden}
      <label class="check small"><input type="checkbox" bind:checked={withHidden} /> {t('showHidden')} ({hidden})</label>
    {/if}
    {#if mergeMode}
      <button class="btn primary" disabled={mergeSel.size < 2} onclick={doMerge}><Icon name="merge" size={20} /> {t('merge')} ({mergeSel.size})</button>
      <button class="btn text" onclick={() => ((mergeMode = false), mergeSel.clear())}>{t('cancel')}</button>
    {:else}
      <button class="btn" onclick={() => (mergeMode = true)}><Icon name="merge" size={20} /> {t('merge')}</button>
    {/if}
  </div>

  {#if loading && !people.length}
    <div class="spinner"></div>
  {:else if !people.length}
    <div class="empty">{t('noPeople')}</div>
  {:else}
    <div class="people-grid">
      {#each people as p (p.id)}
        <div class="person" class:hidden-p={p.isHidden} class:sel={mergeSel.has(p.id)}>
          {#if mergeMode}
            <button
              class="face"
              onclick={() => (mergeSel.has(p.id) ? mergeSel.delete(p.id) : mergeSel.add(p.id))}
              aria-label={p.name || t('unnamedPerson')}
            >
              <img src={personThumbnailUrl(p.id)} alt="" loading="lazy" />
              {#if mergeSel.has(p.id)}<span class="tick"><Icon name="checkCircle" /></span>{/if}
            </button>
          {:else}
            <a class="face" href="/people/{p.id}"><img src={personThumbnailUrl(p.id)} alt="" loading="lazy" /></a>
          {/if}
          <input
            class="name"
            value={p.name}
            placeholder={t('addName')}
            onblur={(e) => rename(p, (e.currentTarget as HTMLInputElement).value.trim())}
            onkeydown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()}
          />
          <div class="row meta">
            <span class="muted small">{t('facesCount', { n: p.faceCount })}</span>
            <button class="icon-btn mini" onclick={() => toggleHidden(p)} title={p.isHidden ? t('unhide') : t('hide')} aria-label={t('hide')}>
              <Icon name={p.isHidden ? 'eye' : 'eyeOff'} size={18} />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .people-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 20px 16px;
  }
  .person {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .person.hidden-p {
    opacity: 0.5;
  }
  .face {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    border-radius: 50%;
    overflow: hidden;
    border: none;
    padding: 0;
    background: var(--placeholder);
    cursor: pointer;
    display: block;
  }
  .person.sel .face {
    outline: 4px solid var(--primary);
    outline-offset: 2px;
  }
  .face img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .tick {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--primary);
    background: #fff;
    border-radius: 50%;
    display: flex;
  }
  .name {
    text-align: center;
    border: none;
    background: transparent;
    min-height: 32px;
    padding: 4px;
    font-weight: 500;
  }
  .meta {
    gap: 2px;
  }
  .mini {
    width: 28px;
    height: 28px;
  }
  @media (max-width: 600px) {
    .people-grid {
      grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
    }
  }
</style>
