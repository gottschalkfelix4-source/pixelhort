<script lang="ts">
  import Icon from './Icon.svelte';
  import { thumbnailUrl, type AssetDto, type ShareAccess } from '../lib/api';
  import type { Selection } from '../lib/selection.svelte';
  import { openViewer, type ViewerContext } from '../lib/state.svelte';
  import { aspectOf, dayKey, dayLabel, formatDuration, justify, thumbhashUrl } from '../lib/utils';
  import { t } from '../lib/i18n';

  let {
    assets,
    selection,
    groupByDay = true,
    share = null,
    viewerAssets,
    viewerOpts = {},
    pickMode = false,
    onchange,
    onopen,
  }: {
    assets: AssetDto[];
    selection?: Selection;
    groupByDay?: boolean;
    share?: ShareAccess | null;
    /** full list used for prev/next in the viewer (defaults to `assets`) */
    viewerAssets?: () => AssetDto[];
    viewerOpts?: Partial<ViewerContext>;
    pickMode?: boolean;
    onchange?: () => void;
    onopen?: (a: AssetDto) => void;
  } = $props();

  let width = $state(0);
  const rowHeight = $derived(width < 600 ? 110 : width < 1000 ? 170 : 220);

  interface Group {
    key: string;
    label: string;
    items: AssetDto[];
    boxes: { left: number; top: number; width: number; height: number }[];
    height: number;
  }

  const groups = $derived.by<Group[]>(() => {
    const out: { key: string; day: string; label: string; items: AssetDto[] }[] = [];
    if (groupByDay) {
      for (const a of assets) {
        const k = dayKey(a.localDateTime);
        const last = out[out.length - 1];
        if (last && last.day === k) last.items.push(a);
        else out.push({ key: `${k}#${out.length}`, day: k, label: dayLabel(a.localDateTime), items: [a] });
      }
    } else {
      out.push({ key: 'all', day: '', label: '', items: assets });
    }
    return out.map((g) => {
      const { boxes, height } = justify(g.items.map(aspectOf), width, rowHeight);
      return { ...g, boxes, height };
    });
  });

  function click(e: MouseEvent, a: AssetDto) {
    if (selection && (pickMode || selection.active || e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      toggle(e, a);
      return;
    }
    open(a);
  }

  function toggle(e: MouseEvent, a: AssetDto) {
    if (!selection) return;
    e.stopPropagation();
    if (e.shiftKey && selection.anchor) {
      const ai = assets.findIndex((x) => x.id === selection.anchor);
      const bi = assets.findIndex((x) => x.id === a.id);
      if (ai >= 0 && bi >= 0) {
        const [s, en] = ai < bi ? [ai, bi] : [bi, ai];
        selection.add(assets.slice(s, en + 1));
        selection.anchor = a.id;
        return;
      }
    }
    selection.toggle(a);
  }

  function open(a: AssetDto) {
    if (onopen) return onopen(a);
    const list = viewerAssets ? viewerAssets() : assets;
    const index = Math.max(
      0,
      list.findIndex((x) => x.id === a.id),
    );
    openViewer({ assets: list, index, share, onChange: onchange, ...viewerOpts });
  }

  function toggleGroup(g: Group) {
    if (!selection) return;
    const all = g.items.every((a) => selection.has(a.id));
    if (all) selection.removeAll(g.items);
    else selection.add(g.items);
  }
</script>

<div class="grid-wrap" bind:clientWidth={width}>
  {#each groups as g (g.key)}
    {#if groupByDay}
      <div class="day-header">
        {#if selection}
          <button
            class="day-check"
            class:visible={selection.active}
            onclick={() => toggleGroup(g)}
            title={t('selectDay')}
            aria-label={t('selectDay')}
          >
            <Icon name={g.items.every((a) => selection.has(a.id)) ? 'checkCircle' : 'circle'} size={20} />
          </button>
        {/if}
        <span>{g.label}</span>
      </div>
    {/if}
    <div class="justified" style="height: {g.height}px">
      {#each g.items as a, i (a.id)}
        {@const b = g.boxes[i]}
        {@const sel = selection?.has(a.id) ?? false}
        {@const hash = thumbhashUrl(a.thumbhash)}
        <div
          class="tile"
          class:selected={sel}
          class:selecting={selection?.active}
          style="left:{b.left}px;top:{b.top}px;width:{b.width}px;height:{b.height}px;{hash
            ? `background-image:url(${hash})`
            : ''}"
          role="button"
          tabindex="0"
          onclick={(e) => click(e, a)}
          onkeydown={(e) => e.key === 'Enter' && open(a)}
        >
          <img
            src={thumbnailUrl(a.id, 'thumbnail', share, a.hasEdits ? a.updatedAt : undefined)}
            alt={a.originalFileName}
            loading="lazy"
            decoding="async"
            draggable="false"
            onload={(e) => (e.currentTarget as HTMLImageElement).classList.add('loaded')}
          />
          <div class="overlay">
            <div class="top-right">
              {#if a.type === 'VIDEO'}
                <span class="pill">{formatDuration(a.duration)} <Icon name="play" size={16} /></span>
              {:else if a.livePhotoVideoId}
                <Icon name="live" size={18} />
              {/if}
              {#if a.projectionType === 'EQUIRECTANGULAR'}<Icon name="sphere" size={18} />{/if}
              {#if a.isPanorama}<Icon name="panorama" size={18} />{/if}
              {#if a.isRaw}<span class="pill">RAW</span>{/if}
            </div>
            {#if a.isFavorite}
              <div class="bottom-left"><Icon name="heart" size={18} /></div>
            {/if}
          </div>
          {#if selection}
            <button class="check" aria-label={t('select')} onclick={(e) => toggle(e, a)}>
              <Icon name={sel ? 'checkCircle' : 'circle'} size={22} />
            </button>
          {/if}
        </div>
      {/each}
    </div>
  {/each}
</div>

<style>
  .grid-wrap {
    width: 100%;
  }
  .day-header {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 0.9rem;
    font-weight: 500;
    padding: 16px 0 8px;
    color: var(--fg);
  }
  .day-check {
    border: none;
    background: none;
    padding: 0;
    color: var(--fg-muted);
    cursor: pointer;
    width: 0;
    overflow: hidden;
    transition: width 0.15s;
    display: inline-flex;
  }
  .day-header:hover .day-check,
  .day-check.visible {
    width: 24px;
  }
  .justified {
    position: relative;
    width: 100%;
  }
  .tile {
    position: absolute;
    overflow: hidden;
    cursor: pointer;
    background-color: var(--placeholder);
    background-size: cover;
    background-position: center;
    transition: transform 0.12s;
    outline: none;
  }
  .tile:focus-visible {
    box-shadow: inset 0 0 0 3px var(--primary);
  }
  .tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    opacity: 0;
    transition: opacity 0.25s;
    user-select: none;
  }
  .tile :global(img.loaded) {
    opacity: 1;
  }
  .tile.selected {
    background: var(--primary-soft) !important;
  }
  .tile.selected img {
    transform: scale(0.86);
    border-radius: 8px;
  }
  .overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    color: #fff;
    filter: drop-shadow(0 0 2px rgba(0, 0, 0, 0.8));
  }
  .top-right {
    position: absolute;
    top: 6px;
    right: 6px;
    display: flex;
    gap: 4px;
    align-items: center;
    font-size: 0.75rem;
    font-weight: 500;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .bottom-left {
    position: absolute;
    left: 6px;
    bottom: 6px;
  }
  .check {
    position: absolute;
    top: 4px;
    left: 4px;
    border: none;
    background: none;
    padding: 2px;
    color: #fff;
    cursor: pointer;
    opacity: 0;
    filter: drop-shadow(0 0 2px rgba(0, 0, 0, 0.6));
    transition: opacity 0.12s;
    display: flex;
  }
  .tile::before {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(rgba(0, 0, 0, 0.35), transparent 30%);
    opacity: 0;
    transition: opacity 0.12s;
    z-index: 1;
    pointer-events: none;
  }
  .tile:hover::before,
  .tile.selecting::before {
    opacity: 1;
  }
  .tile:hover .check,
  .tile.selecting .check,
  .tile.selected .check {
    opacity: 1;
    z-index: 2;
  }
  .tile.selected .check {
    color: var(--primary);
    filter: none;
  }
  .tile.selected::before {
    opacity: 0;
  }
  @media (hover: none) {
    .check {
      opacity: 0.85;
    }
  }
</style>
