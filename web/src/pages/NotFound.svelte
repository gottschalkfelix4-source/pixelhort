<script lang="ts">
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import { t } from '../lib/i18n';
</script>

<div class="page">
  <div class="spacer-top"></div>
  <EmptyState icon="explore" title="404" text={t('pageNotFound')}>
    <a class="btn primary" href="/photos"><Icon name="photo" size={20} /> {t('backToPhotos')}</a>
  </EmptyState>
</div>

<style>
  .spacer-top {
    height: 48px;
  }
</style>
