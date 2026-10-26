<script lang="ts">
  import Timeline from '../components/Timeline.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import Icon from '../components/Icon.svelte';
  import { api, type TagDto } from '../lib/api';
  import { Selection } from '../lib/selection.svelte';
  import { t } from '../lib/i18n';

  let { params }: { params: { id: string } } = $props();
  const selection = new Selection();
  let tags = $state<TagDto[]>([]);

  $effect(() => {
    api
      .tags()
      .then((r) => (tags = r))
      .catch(() => {});
  });

  const tag = $derived(tags.find((x) => x.id === params.id));
  const children = $derived(tags.filter((x) => x.parentId === params.id));
</script>

<div class="page">
  <div class="page-header">
    <a class="icon-btn" href="/tags" aria-label={t('back')}><Icon name="back" /></a>
    <h1>
      {#if tag}<span class="dot" style="background: {tag.color || 'var(--fg-muted)'}"></span>{tag.value}{:else}{t('navTags')}{/if}
    </h1>
  </div>
  {#if children.length}
    <div class="row wrap children">
      {#each children as c (c.id)}
        <a class="chip" href="/tags/{c.id}">{c.name || c.value} <span class="muted">{c.assetCount}</span></a>
      {/each}
    </div>
  {/if}
  <Timeline filters={{ tagId: params.id }} {selection} emptyText={t('nothingHere')} />
</div>
<SelectionBar {selection} context="tag" />

<style>
  .dot {
    display: inline-block;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    margin-right: 10px;
  }
  .children {
    margin-bottom: 8px;
  }
</style>
