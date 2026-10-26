<script lang="ts">
  import Timeline from '../components/Timeline.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import { Selection } from '../lib/selection.svelte';
  import { t } from '../lib/i18n';

  const selection = new Selection();
</script>

<div class="page">
  <div class="page-header"><h1>{t('navFavorites')}</h1></div>
  <Timeline filters={{ isFavorite: true }} {selection} emptyText={t('favoritesEmpty')} />
</div>
<SelectionBar {selection} context="favorites" />
