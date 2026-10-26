<script lang="ts">
  import Icon from '../components/Icon.svelte';
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
    <div class="empty">{t('noTags')}</div>
  {:else}
    <ul class="tree">
      {#each tree as n (n.tag.id)}
        <li style="padding-left: {n.depth * 24 + 8}px">
          <input
            type="color"
            class="color"
            value={n.tag.color || '#9aa0a6'}
            onchange={(e) => setColor(n.tag, (e.currentTarget as HTMLInputElement).value)}
            title={t('color')}
          />
          <a href="/tags/{n.tag.id}" class="name">{n.tag.name || n.tag.value}</a>
          <span class="muted small">{n.tag.assetCount}</span>
          <span class="spacer"></span>
          <button class="icon-btn" title={t('newTag')} aria-label={t('newTag')} onclick={() => create(n.tag)}><Icon name="add" size={20} /></button>
          <button class="icon-btn" title={t('delete')} aria-label={t('delete')} onclick={() => remove(n.tag)}><Icon name="delete" size={20} /></button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .tree {
    list-style: none;
    padding: 0;
    margin: 0;
    max-width: 720px;
  }
  .tree li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 48px;
    border-bottom: 1px solid var(--border);
  }
  .color {
    width: 22px;
    height: 22px;
    border: none;
    padding: 0;
    background: none;
    cursor: pointer;
    border-radius: 50%;
  }
  .name {
    color: var(--fg);
    font-weight: 500;
  }
</style>
