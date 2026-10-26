<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import EmptyState from '../components/EmptyState.svelte';
  import { api, type TagDto } from '../lib/api';
  import { confirmDialog, promptDialog, toastError } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  let tags = $state<TagDto[]>([]);
  let loading = $state(true);

  async function load() {
    loading = true;
    try {
      tags = await api.tags();
    } catch (e) {
      toastError(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    load();
  });

  interface Node {
    tag: TagDto;
    depth: number;
  }
  // Flattened tree (depth-first), built from parentId (fallback: value path).
  const tree = $derived.by<Node[]>(() => {
    const byParent = new Map<string | null, TagDto[]>();
    const ids = new Set(tags.map((x) => x.id));
    for (const tag of tags) {
      const p = tag.parentId && ids.has(tag.parentId) ? tag.parentId : null;
      if (!byParent.has(p)) byParent.set(p, []);
      byParent.get(p)!.push(tag);
    }
    for (const list of byParent.values()) list.sort((a, b) => a.value.localeCompare(b.value));
    const out: Node[] = [];
    const walk = (p: string | null, depth: number) => {
      for (const tag of byParent.get(p) ?? []) {
        out.push({ tag, depth });
        walk(tag.id, depth + 1);
      }
    };
    walk(null, 0);
    return out;
  });

  async function create(parent?: TagDto) {
    const v = await promptDialog(t('newTag'), {
      message: t('tagValueHint'),
      value: parent ? `${parent.value}/` : '',
      confirmLabel: t('create'),
    });
    if (!v?.trim()) return;
    try {
      await api.createTag(v.trim().replace(/^\/+|\/+$/g, ''));
      load();
    } catch (e) {
      toastError(e);
    }
  }

  async function setColor(tag: TagDto, color: string) {
    try {
      const res = await api.updateTag(tag.id, color);
      tag.color = res.color;
    } catch (e) {
      toastError(e);
    }
  }

  async function remove(tag: TagDto) {
    if (!(await confirmDialog(t('delete'), { message: t('deleteTagConfirm', { name: tag.value }), danger: true, confirmLabel: t('delete') })))
      return;
    try {
      await api.deleteTag(tag.id);
      load();
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{t('navTags')}</h1>
    <button class="btn primary" onclick={() => create()}><Icon name="add" size={20} /> {t('newTag')}</button>
  </div>
  {#if loading && !tags.length}
    <div class="spinner"></div>
  {:else if !tags.length}
    <EmptyState icon="tag" title={t('noTags')} text={t('noTagsHint')}>
      <button class="btn primary" onclick={() => create()}><Icon name="add" size={20} /> {t('newTag')}</button>
    </EmptyState>
  {:else}
    <ul class="tree">
      {#each tree as n (n.tag.id)}
        <li style="--depth: {n.depth}">
          {#if n.depth}<span class="branch" aria-hidden="true"></span>{/if}
          <label class="swatch" style="--c: {n.tag.color || '#9aa0a6'}" title={t('color')}>
            <Icon name="tag" size={20} />
            <input
              type="color"
              value={n.tag.color || '#9aa0a6'}
              onchange={(e) => setColor(n.tag, (e.currentTarget as HTMLInputElement).value)}
              aria-label={t('color')}
            />
          </label>
          <a href="/tags/{n.tag.id}" class="name">{n.tag.name || n.tag.value}</a>
          <span class="badge">{n.tag.assetCount}</span>
          <span class="spacer"></span>
          <div class="acts">
            <button class="icon-btn" title={t('newTag')} aria-label={t('newTag')} onclick={() => create(n.tag)}><Icon name="add" size={20} /></button>
            <button class="icon-btn" title={t('delete')} aria-label={t('delete')} onclick={() => remove(n.tag)}><Icon name="delete" size={20} /></button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .tree {
    list-style: none;
    padding: 6px;
    margin: 0;
    max-width: 760px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-elev);
  }
  .tree li {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 0 6px 0 calc(10px + var(--depth) * 28px);
    border-radius: 12px;
    position: relative;
    transition: background 0.12s;
  }
  .tree li:hover {
    background: var(--bg-hover);
  }
  .branch {
    position: absolute;
    left: calc(var(--depth) * 28px - 6px);
    top: 0;
    width: 12px;
    height: 50%;
    border-left: 1.5px solid var(--border-strong);
    border-bottom: 1.5px solid var(--border-strong);
    border-bottom-left-radius: 6px;
  }
  .swatch {
    position: relative;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 16%, transparent);
    cursor: pointer;
    flex-shrink: 0;
  }
  .swatch input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
    width: 100%;
    height: 100%;
  }
  .name {
    color: var(--fg);
    font-weight: 600;
  }
  .name:hover {
    color: var(--primary);
  }
  .acts {
    display: flex;
    opacity: 0.4;
    transition: opacity 0.15s;
  }
  .tree li:hover .acts {
    opacity: 1;
  }
  @media (hover: none) {
    .acts {
      opacity: 1;
    }
  }
</style>
