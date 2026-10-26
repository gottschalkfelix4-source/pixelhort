<script lang="ts">
  import Users from './admin/Users.svelte';
  import Jobs from './admin/Jobs.svelte';
  import Libraries from './admin/Libraries.svelte';
  import Config from './admin/Config.svelte';
  import Backups from './admin/Backups.svelte';
  import Stats from './admin/Stats.svelte';
  import Icon, { type IconName } from '../components/Icon.svelte';
  import { auth } from '../lib/state.svelte';
  import { t, type MessageKey } from '../lib/i18n';

  let { params }: { params: { tab?: string } } = $props();

  const tabs: { id: string; label: MessageKey; icon: IconName }[] = [
    { id: 'users', label: 'users', icon: 'people' },
    { id: 'jobs', label: 'jobs', icon: 'work' },
    { id: 'libraries', label: 'libraries', icon: 'folder' },
    { id: 'config', label: 'systemConfig', icon: 'settings' },
    { id: 'backups', label: 'backups', icon: 'backup' },
    { id: 'stats', label: 'statistics', icon: 'chart' },
  ];
  const tab = $derived(tabs.some((x) => x.id === params.tab) ? params.tab! : 'users');
</script>

<div class="page">
  <div class="page-header"><h1>{t('admin')}</h1></div>
  {#if !auth.user?.isAdmin}
    <div class="empty">403</div>
  {:else}
    <nav class="tabs">
      {#each tabs as x (x.id)}
        <a href="/admin/{x.id}" class:active={tab === x.id}><Icon name={x.icon} size={18} /> {t(x.label)}</a>
      {/each}
    </nav>
    {#if tab === 'users'}<Users />
    {:else if tab === 'jobs'}<Jobs />
    {:else if tab === 'libraries'}<Libraries />
    {:else if tab === 'config'}<Config />
    {:else if tab === 'backups'}<Backups />
    {:else}<Stats />{/if}
  {/if}
</div>

<style>
  .tabs a {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
