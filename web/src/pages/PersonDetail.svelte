<script lang="ts">
  import Timeline from '../components/Timeline.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import Pickers from '../components/Pickers.svelte';
  import Icon from '../components/Icon.svelte';
  import { api, personThumbnailUrl, type PersonDto } from '../lib/api';
  import { Selection } from '../lib/selection.svelte';
  import { bumpAssets, confirmDialog, toast, toastError } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  let { params }: { params: { id: string } } = $props();
  const selection = new Selection();
  let person = $state<PersonDto | null>(null);
  let name = $state('');
  let birthDate = $state('');
  let merging = $state(false);

  async function load() {
    try {
      person = await api.person(params.id);
      name = person.name;
      birthDate = person.birthDate ?? '';
    } catch (e) {
      toastError(e);
    }
  }
  $effect(() => {
    load();
  });

  async function update(p: Parameters<typeof api.updatePerson>[1]) {
    try {
      person = await api.updatePerson(params.id, p);
      toast(t('saved'), 'success');
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <a class="icon-btn" href="/people" aria-label={t('back')}><Icon name="back" /></a>
    {#if person}
      <img class="avatar" src={personThumbnailUrl(person.id)} alt="" />
      <input
        class="name"
        bind:value={name}
        placeholder={t('addName')}
        onblur={() => person && name.trim() !== person.name && update({ name: name.trim() })}
        onkeydown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()}
      />
      <label class="field bd"
        >{t('birthDate')}<input
          type="date"
          bind:value={birthDate}
          onchange={() => update({ birthDate: birthDate || null })}
        /></label
      >
      <button class="btn" onclick={() => update({ isHidden: !person!.isHidden })}>
        <Icon name={person.isHidden ? 'eye' : 'eyeOff'} size={20} />
        {person.isHidden ? t('unhide') : t('hide')}
      </button>
      <button class="btn" onclick={() => (merging = true)}><Icon name="merge" size={20} /> {t('merge')}</button>
    {/if}
  </div>
  {#if person}<p class="muted small">{t('facesCount', { n: person.faceCount })}</p>{/if}
  <Timeline filters={{ personId: params.id }} {selection} emptyText={t('nothingHere')} />
</div>
<SelectionBar {selection} context="person" />

{#if merging && person}
  <Pickers
    kind="person"
    title={t('mergeInto', { name: person.name || t('unnamedPerson') })}
    exclude={[person.id]}
    allowCreate={false}
    onclose={() => (merging = false)}
    onpick={async (p) => {
      merging = false;
      if (p.kind !== 'person' || !p.person) return;
      if (!(await confirmDialog(t('merge'), { message: t('mergeConfirm', { n: 2 }) }))) return;
      try {
        person = await api.mergePeople(params.id, [p.person.id]);
        bumpAssets();
      } catch (e) {
        toastError(e);
      }
    }}
  />
{/if}

<style>
  .avatar {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    object-fit: cover;
  }
  .name {
    flex: 1;
    min-width: 160px;
    font-size: 1.5rem;
    border: none;
    background: transparent;
  }
  .bd {
    width: 160px;
  }
</style>
