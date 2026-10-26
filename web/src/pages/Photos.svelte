<script lang="ts">
  import Timeline from '../components/Timeline.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import Memories from '../components/Memories.svelte';
  import Icon from '../components/Icon.svelte';
  import { api } from '../lib/api';
  import { Selection } from '../lib/selection.svelte';
  import { prefs, ui, writePref } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  const selection = new Selection();
  const filters = $derived(ui.withPartners ? { withPartners: true } : {});
  // The partner toggle only makes sense when someone shares their photos with us.
  let hasPartners = $state(false);

  $effect(() => {
    api
      .partners('shared-with')
      .then((p) => (hasPartners = p.length > 0))
      .catch(() => {});
  });

  function togglePartners() {
    ui.withPartners = !ui.withPartners;
    writePref('ph_with_partners', String(ui.withPartners));
  }
</script>

<div class="page">
  <Timeline {filters} {selection}>
    {#snippet header()}
      {#if prefs.value.memories.enabled}<Memories />{/if}
      {#if hasPartners || ui.withPartners}
        <div class="row toolbar">
          <span class="spacer"></span>
          <button class="chip" class:active={ui.withPartners} onclick={togglePartners} aria-pressed={ui.withPartners}>
            <Icon name={ui.withPartners ? 'check' : 'people'} size={18} />
            {t('showPartners')}
          </button>
        </div>
      {/if}
    {/snippet}
  </Timeline>
</div>
<SelectionBar {selection} context="timeline" />

<style>
  .page {
    padding-top: 8px;
  }
  .toolbar {
    margin-top: 8px;
  }
</style>
