<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import { api, thumbnailUrl, type DuplicateGroup } from '../lib/api';
  import { bumpAssets, openViewer, toast, toastError } from '../lib/state.svelte';
  import { formatBytes, formatLocal } from '../lib/utils';
  import { t } from '../lib/i18n';

  let groups = $state<DuplicateGroup[]>([]);
  let keep = $state<Record<string, string[]>>({});
  let loading = $state(true);

  async function load() {
    loading = true;
    try {
      groups = await api.duplicates();
      const k: Record<string, string[]> = {};
      for (const g of groups) {
        // default: keep the largest file (likely the original quality)
        const best = [...g.assets].sort((a, b) => b.fileSize - a.fileSize || (b.width ?? 0) * (b.height ?? 0) - (a.width ?? 0) * (a.height ?? 0))[0];
        k[g.duplicateId] = best ? [best.id] : [];
      }
      keep = k;
    } catch (e) {
      toastError(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    load();
  });

  function toggle(gid: string, id: string) {
    const list = keep[gid] ?? [];
    keep[gid] = list.includes(id) ? list.filter((x) => x !== id) : [...list, id];
  }

  async function resolve(g: DuplicateGroup) {
    const k = keep[g.duplicateId] ?? [];
    const trash = g.assets.map((a) => a.id).filter((id) => !k.includes(id));
    try {
      await api.resolveDuplicate(g.duplicateId, k, trash);
      groups = groups.filter((x) => x.duplicateId !== g.duplicateId);
      if (trash.length) toast(t('movedToTrash'));
      bumpAssets();
    } catch (e) {
      toastError(e);
    }
  }

  async function dismiss(g: DuplicateGroup) {
    try {
      await api.dismissDuplicate(g.duplicateId);
      groups = groups.filter((x) => x.duplicateId !== g.duplicateId);
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{t('duplicatesTitle')}</h1>
    <span class="muted small">{groups.length}</span>
  </div>
  <p class="muted">{t('duplicatesHint')}</p>
  {#if loading && !groups.length}
    <div class="spinner"></div>
  {:else if !groups.length}
    <div class="empty">{t('noDuplicates')}</div>
  {:else}
    {#each groups as g (g.duplicateId)}
      <div class="card group">
        <div class="items">
          {#each g.assets as a, i (a.id)}
            {@const kept = keep[g.duplicateId]?.includes(a.id)}
            <div class="item" class:kept>
              <button class="thumb" onclick={() => openViewer({ assets: g.assets, index: i, readOnly: true })}>
                <img src={thumbnailUrl(a.id)} alt="" loading="lazy" />
              </button>
              <label class="check"><input type="checkbox" checked={kept} onchange={() => toggle(g.duplicateId, a.id)} /> {t('keep')}</label>
              <div class="small break">{a.originalFileName}</div>
              <div class="muted small">
                {formatBytes(a.fileSize)}{a.width ? ` · ${a.width}×${a.height}` : ''}<br />
                {formatLocal(a.localDateTime, { dateStyle: 'medium', timeStyle: 'short' })}
              </div>
            </div>
          {/each}
        </div>
        <div class="row">
          <span class="spacer"></span>
          <button class="btn text" onclick={() => dismiss(g)}>{t('notDuplicates')}</button>
          <button class="btn primary" onclick={() => resolve(g)}><Icon name="check" size={20} /> {t('keepSelected')}</button>
        </div>
      </div>
    {/each}
  {/if}
</div>

<style>
  .group {
    margin-bottom: 16px;
    padding: 16px;
  }
  .items {
    display: flex;
    gap: 16px;
    overflow-x: auto;
    padding-bottom: 8px;
  }
  .item {
    width: 180px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    opacity: 0.6;
  }
  .item.kept {
    opacity: 1;
  }
  .thumb {
    aspect-ratio: 1;
    border-radius: 10px;
    overflow: hidden;
    border: 3px solid transparent;
    padding: 0;
    cursor: pointer;
    background: var(--placeholder);
  }
  .item.kept .thumb {
    border-color: var(--primary);
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .break {
    word-break: break-all;
  }
</style>
