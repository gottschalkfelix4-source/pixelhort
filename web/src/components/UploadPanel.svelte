<script lang="ts">
  import Icon from './Icon.svelte';
  import { uploads, clearFinished, type UploadStatus } from '../lib/upload.svelte';
  import { formatBytes } from '../lib/utils';
  import { t, type MessageKey } from '../lib/i18n';

  let collapsed = $state(false);

  const labels: Record<UploadStatus, MessageKey> = {
    hashing: 'uploadHashing',
    pending: 'uploadPending',
    uploading: 'uploadUploading',
    done: 'uploadDone',
    duplicate: 'uploadDuplicate',
    error: 'uploadError',
  };
  const overall = $derived(
    uploads.items.length ? uploads.items.reduce((s, i) => s + (i.status === 'uploading' ? i.progress : i.status === 'pending' || i.status === 'hashing' ? 0 : 1), 0) / uploads.items.length : 0,
  );
</script>

{#if uploads.panelOpen && uploads.items.length}
  <div class="panel" class:collapsed>
    <header>
      <button class="icon-btn" onclick={() => (collapsed = !collapsed)} aria-label={t('uploads')}>
        <Icon name={collapsed ? 'chevronRight' : 'chevronDown'} />
      </button>
      <div class="title">
        <strong>{t('uploads')}</strong>
        <small class="muted"
          >{t('uploadSummary', { done: uploads.stats.done, total: uploads.stats.total, dup: uploads.stats.dup, err: uploads.stats.err })}</small
        >
      </div>
      {#if !uploads.active}
        <button class="icon-btn" onclick={clearFinished} aria-label={t('clearFinished')} title={t('clearFinished')}><Icon name="close" /></button>
      {/if}
    </header>
    <div class="bar"><div style="width: {overall * 100}%"></div></div>
    {#if !collapsed}
      <ul>
        {#each uploads.items as it (it.id)}
          <li class={it.status}>
            <div class="name" title={it.name}>{it.name}</div>
            <div class="meta">
              <span>{formatBytes(it.size)}</span>
              <span class="status" title={it.error}>
                {#if it.status === 'uploading'}{Math.round(it.progress * 100)} %{:else}{t(labels[it.status])}{/if}
              </span>
            </div>
            {#if it.status === 'uploading'}
              <div class="bar small"><div style="width: {it.progress * 100}%"></div></div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .panel {
    position: fixed;
    right: 16px;
    bottom: 16px;
    width: 360px;
    max-width: calc(100vw - 32px);
    background: var(--bg-elev);
    border-radius: 12px;
    box-shadow: var(--shadow);
    z-index: 900;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 6px 6px 4px;
  }
  .title {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .bar {
    height: 3px;
    background: var(--bg-soft);
  }
  .bar div {
    height: 100%;
    background: var(--primary);
    transition: width 0.2s;
  }
  .bar.small {
    margin-top: 4px;
    border-radius: 2px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px 0;
    max-height: 300px;
    overflow-y: auto;
  }
  li {
    padding: 6px 16px;
    font-size: 0.85rem;
  }
  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    display: flex;
    justify-content: space-between;
    color: var(--fg-muted);
    font-size: 0.78rem;
  }
  li.done .status {
    color: var(--success);
  }
  li.duplicate .status {
    color: var(--warning);
  }
  li.error .status {
    color: var(--danger);
  }
</style>
