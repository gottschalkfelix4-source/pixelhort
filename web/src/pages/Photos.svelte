<script lang="ts">
  import Timeline from '../components/Timeline.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import Memories from '../components/Memories.svelte';
  import { Selection } from '../lib/selection.svelte';
  import { ui, writePref } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  const selection = new Selection();
  const filters = $derived(ui.withPartners ? { withPartners: true } : {});

  function togglePartners() {
    ui.withPartners = !ui.withPartners;
    writePref('ph_with_partners', String(ui.withPartners));
  }
</script>

<div class="page">
  <Timeline {filters} {selection}>
    {#snippet header()}
      <Memories />
      <div class="row toolbar">
        <span class="spacer"></span>
        <label class="check small muted"><input type="checkbox" checked={ui.withPartners} onchange={togglePartners} /> {t('showPartners')}</label>
      </div>
    {/snippet}
  </Timeline>
</div>
<SelectionBar {selection} context="timeline" />

<style>
  .toolbar {
    margin-top: 4px;
  }
</style>
