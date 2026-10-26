<script lang="ts">
  import AssetGrid from '../components/AssetGrid.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import Icon from '../components/Icon.svelte';
  import { api, type AssetDto } from '../lib/api';
  import { Selection } from '../lib/selection.svelte';
  import { auth, bumpAssets, confirmDialog, toast, toastError, ui } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  const selection = new Selection();
  let assets = $state<AssetDto[]>([]);
  let loading = $state(true);
  let days = $state<number | null>(null);

  async function load() {
    loading = true;
    try {
      assets = await api.trash();
    } catch (e) {
      toastError(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void ui.assetsVersion;
    load();
  });

  $effect(() => {
    if (auth.serverInfo?.trashDays !== undefined) days = auth.serverInfo.trashDays;
  });

  async function restoreAll() {
    try {
      await api.restoreTrash();
      toast(t('restore'), 'success');
      bumpAssets();
    } catch (e) {
      toastError(e);
    }
  }

  async function empty() {
    if (!(await confirmDialog(t('emptyTrash'), { message: t('emptyTrashConfirm'), confirmLabel: t('emptyTrash'), danger: true }))) return;
    try {
      await api.emptyTrash();
      bumpAssets();
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{t('navTrash')}</h1>
    {#if assets.length}
      <button class="btn" onclick={restoreAll}><Icon name="restore" size={20} /> {t('restoreAll')}</button>
      <button class="btn danger" onclick={empty}><Icon name="delete" size={20} /> {t('emptyTrash')}</button>
    {/if}
  </div>
  {#if days}<p class="muted small">{t('trashHint', { n: days })}</p>{/if}
  {#if loading && !assets.length}
    <div class="spinner"></div>
  {:else if !assets.length}
    <div class="empty">{t('trashEmpty')}</div>
  {:else}
    <AssetGrid {assets} {selection} groupByDay={false} onchange={load} />
  {/if}
</div>
<SelectionBar {selection} context="trash" />
