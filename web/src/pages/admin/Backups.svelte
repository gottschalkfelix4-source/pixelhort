<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import { api, backupDownloadUrl, getToken, type BackupDto } from '../../lib/api';
  import { confirmDialog, toast, toastError } from '../../lib/state.svelte';
  import { formatAbs, formatBytes, triggerDownload } from '../../lib/utils';
  import { t } from '../../lib/i18n';

  let backups = $state<BackupDto[]>([]);

  async function load() {
    try {
      backups = (await api.backups()).sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1));
    } catch (e) {
      toastError(e);
    }
  }
  $effect(() => {
    load();
  });

  async function create() {
    try {
      await api.createBackup();
      toast(t('backupStarted'), 'success');
      setTimeout(load, 4000);
    } catch (e) {
      toastError(e);
    }
  }

  async function download(b: BackupDto) {
    // Cookie auth covers plain links; fall back to a fetched blob with the bearer token.
    const url = backupDownloadUrl(b.name);
    const token = getToken();
    if (!token) return triggerDownload(url, b.name);
    try {
      const res = await fetch(url, { headers: { Authorization: `Bearer ${token}` }, credentials: 'same-origin' });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const blob = await res.blob();
      const obj = URL.createObjectURL(blob);
      triggerDownload(obj, b.name);
      setTimeout(() => URL.revokeObjectURL(obj), 10000);
    } catch (e) {
      toastError(e);
    }
  }

  async function remove(b: BackupDto) {
    if (!(await confirmDialog(t('delete'), { message: b.name, danger: true, confirmLabel: t('delete') }))) return;
    try {
      await api.deleteBackup(b.name);
      load();
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="row head">
  <button class="icon-btn" onclick={load} aria-label={t('refresh')}><Icon name="refresh" /></button>
  <span class="spacer"></span>
  <button class="btn primary" onclick={create}><Icon name="backup" size={20} /> {t('createBackup')}</button>
</div>
{#if !backups.length}
  <div class="empty">{t('noBackups')}</div>
{:else}
  <div class="table-wrap">
    <table class="table">
      <thead><tr><th>{t('name')}</th><th>{t('size')}</th><th>{t('createdAt')}</th><th></th></tr></thead>
      <tbody>
        {#each backups as b (b.name)}
          <tr>
            <td class="name">{b.name}</td>
            <td>{formatBytes(b.size)}</td>
            <td>{formatAbs(b.createdAt)}</td>
            <td class="acts">
              <button class="icon-btn" onclick={() => download(b)} aria-label={t('download')}><Icon name="download" size={20} /></button>
              <button class="icon-btn" onclick={() => remove(b)} aria-label={t('delete')}><Icon name="delete" size={20} /></button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

<style>
  .head {
    margin-bottom: 12px;
  }
  .name {
    word-break: break-all;
    font-family: ui-monospace, monospace;
    font-size: 0.85rem;
  }
  .acts {
    white-space: nowrap;
    text-align: right;
  }
</style>
