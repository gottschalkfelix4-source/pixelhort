<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import { api, type ServerStatistics } from '../../lib/api';
  import { auth, toastError } from '../../lib/state.svelte';
  import { formatBytes } from '../../lib/utils';
  import { t } from '../../lib/i18n';

  let stats = $state<ServerStatistics | null>(null);

  $effect(() => {
    api
      .serverStatistics()
      .then((s) => (stats = s))
      .catch(toastError);
  });

  const maxUsage = $derived(Math.max(1, ...(stats?.users.map((u) => u.usageBytes) ?? [1])));
</script>

{#if !stats}
  <div class="spinner"></div>
{:else}
  <div class="tiles">
    <div class="card tile"><span class="ic"><Icon name="photo" /></span><span class="muted">{t('photos')}</span><strong>{stats.photos.toLocaleString()}</strong></div>
    <div class="card tile"><span class="ic"><Icon name="video" /></span><span class="muted">{t('videos')}</span><strong>{stats.videos.toLocaleString()}</strong></div>
    <div class="card tile"><span class="ic"><Icon name="cloud" /></span><span class="muted">{t('totalUsage')}</span><strong>{formatBytes(stats.usageBytes)}</strong></div>
    {#if auth.serverInfo}
      <div class="card tile"><span class="ic"><Icon name="info" /></span><span class="muted">{t('version')}</span><strong>{auth.serverInfo.version}</strong></div>
    {/if}
  </div>
  <div class="table-wrap">
    <table class="table">
      <thead><tr><th>{t('users')}</th><th>{t('photos')}</th><th>{t('videos')}</th><th>{t('usage')}</th><th class="barcol"></th></tr></thead>
      <tbody>
        {#each stats.users as u (u.userId)}
          <tr>
            <td><strong>{u.name}</strong></td>
            <td>{u.photos.toLocaleString()}</td>
            <td>{u.videos.toLocaleString()}</td>
            <td>{formatBytes(u.usageBytes)}</td>
            <td class="barcol"><div class="bar"><div style="width: {(u.usageBytes / maxUsage) * 100}%"></div></div></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 16px;
    margin-bottom: 24px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 20px;
  }
  .ic {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--primary-soft);
    color: var(--primary);
    margin-bottom: 12px;
  }
  .tile .muted {
    font-size: 0.85rem;
    font-weight: 500;
  }
  .tile strong {
    font-size: 1.9rem;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .barcol {
    width: 30%;
  }
  .bar {
    height: 8px;
    background: var(--bg-soft);
    border-radius: 4px;
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    border-radius: 4px;
    background: var(--primary);
  }
</style>
