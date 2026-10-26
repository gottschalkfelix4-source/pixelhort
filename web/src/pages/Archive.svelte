<script lang="ts">
  import Timeline from '../components/Timeline.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import { Selection } from '../lib/selection.svelte';
  import { t } from '../lib/i18n';

  const selection = new Selection();
</script>

<div class="page">
  <div class="page-header"><h1>{t('navArchive')}</h1></div>
  <Timeline filters={{ visibility: 'archive' }} {selection} emptyText={t('archiveEmpty')} />
</div>
<SelectionBar {selection} context="archive" />
