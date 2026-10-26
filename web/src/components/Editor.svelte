<script lang="ts">
  import Icon from './Icon.svelte';
  import { api, originalUrl, thumbnailUrl, type AssetDto, type EditFilter, type EditsDto } from '../lib/api';
  import { confirmDialog, toast, toastError } from '../lib/state.svelte';
  import { isWebImage } from '../lib/utils';
  import { t, type MessageKey } from '../lib/i18n';

  let { asset, onclose, onsaved }: { asset: AssetDto; onclose: () => void; onsaved: (a: AssetDto, copy: boolean) => void } =
    $props();

  type Tab = 'crop' | 'adjust' | 'filters';
  type Crop = { x: number; y: number; width: number; height: number };

  let tab = $state<Tab>('crop');
  let rotate = $state<0 | 90 | 180 | 270>(0);
  let flipH = $state(false);
  let flipV = $state(false);
  let brightness = $state(0);
  let contrast = $state(0);
  let saturation = $state(0);
  let filter = $state<EditFilter | null>(null);
  let crop = $state<Crop>({ x: 0, y: 0, width: 1, height: 1 });
  let aspect = $state<number | null>(null); // null = free
  let saveAsCopy = $state(false);
  let busy = $state(false);
  let hint = $state(false);

  let img = $state<HTMLImageElement | null>(null);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let stage = $state<HTMLDivElement | null>(null);
  let frame = $state<HTMLDivElement | null>(null);
  let stageW = $state(800);
  let stageH = $state(600);

  // Load source image + existing edits.
  $effect(() => {
    const a = asset;
    let src = thumbnailUrl(a.id, 'preview');
    if (a.hasEdits) {
      if (isWebImage(a.originalMimeType)) src = originalUrl(a.id);
      else hint = true;
    }
    const im = new Image();
    im.onload = () => (img = im);
    im.onerror = () => toast(t('noPreview'), 'error');
    im.src = src;
    api
      .getEdits(a.id)
      .then((e) => {
        if (!e) return;
        const num = (v: unknown) => (typeof v === 'number' && isFinite(v) ? v : 0);
        rotate = ([0, 90, 180, 270] as const).find((r) => r === e.rotate) ?? 0;
        flipH = !!e.flipHorizontal;
        flipV = !!e.flipVertical;
        brightness = num(e.brightness);
        contrast = num(e.contrast);
        saturation = num(e.saturation);
        filter = e.filter ?? null;
        crop = e.crop && typeof e.crop.width === 'number' ? e.crop : { x: 0, y: 0, width: 1, height: 1 };
      })
      .catch(() => {});
  });

  const rotated = $derived(rotate % 180 !== 0);
  const srcW = $derived(img ? (rotated ? img.naturalHeight : img.naturalWidth) : 1);
  const srcH = $derived(img ? (rotated ? img.naturalWidth : img.naturalHeight) : 1);

  // Draw rotated / flipped image to the canvas (crop & colour are previewed via overlay / CSS filter).
  $effect(() => {
    const c = canvas;
    const im = img;
    if (!c || !im) return;
    const maxW = Math.max(100, stageW - 48);
    const maxH = Math.max(100, stageH - 48);
    const scale = Math.min(maxW / srcW, maxH / srcH, 1600 / Math.max(srcW, srcH));
    c.width = Math.round(srcW * scale);
    c.height = Math.round(srcH * scale);
    const ctx = c.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.translate(c.width / 2, c.height / 2);
    ctx.scale(flipH ? -1 : 1, flipV ? -1 : 1);
    ctx.rotate((rotate * Math.PI) / 180);
    const w = im.naturalWidth * scale;
    const h = im.naturalHeight * scale;
    ctx.drawImage(im, -w / 2, -h / 2, w, h);
  });

  const FILTERS: { id: EditFilter | null; label: MessageKey; css: string }[] = [
    { id: null, label: 'filterNone', css: '' },
    { id: 'bw', label: 'filterBw', css: 'grayscale(1)' },
    { id: 'sepia', label: 'filterSepia', css: 'sepia(0.85)' },
    { id: 'warm', label: 'filterWarm', css: 'sepia(0.3) saturate(1.3) hue-rotate(-12deg)' },
    { id: 'cool', label: 'filterCool', css: 'saturate(0.9) hue-rotate(18deg) brightness(1.03)' },
    { id: 'vintage', label: 'filterVintage', css: 'sepia(0.45) contrast(0.88) brightness(1.06) saturate(0.8)' },
  ];

  const cssFilter = $derived(
    `brightness(${1 + brightness / 100}) contrast(${1 + contrast / 100}) saturate(${1 + saturation / 100}) ${
      FILTERS.find((f) => f.id === filter)?.css ?? ''
    }`,
  );

  const ASPECTS: { label: string; key?: MessageKey; value: number | null | 'orig' }[] = [
    { label: '', key: 'aspectFree', value: null },
    { label: '', key: 'aspectOriginal', value: 'orig' },
    { label: '1:1', value: 1 },
    { label: '4:3', value: 4 / 3 },
    { label: '3:4', value: 3 / 4 },
    { label: '3:2', value: 3 / 2 },
    { label: '16:9', value: 16 / 9 },
    { label: '9:16', value: 9 / 16 },
  ];
  let aspectChoice = $state<number | null | 'orig'>(null);

  function chooseAspect(v: number | null | 'orig') {
    aspectChoice = v;
    aspect = v === 'orig' ? srcW / srcH : v;
    if (aspect) {
      // largest centered crop with that pixel aspect ratio
      const imgAspect = srcW / srcH;
      let w = 1;
      let h = imgAspect / aspect;
      if (h > 1) {
        h = 1;
        w = aspect / imgAspect;
      }
      crop = { x: (1 - w) / 2, y: (1 - h) / 2, width: w, height: h };
    }
  }

  function doRotate() {
    rotate = ((rotate + 90) % 360) as 0 | 90 | 180 | 270;
    crop = { x: 0, y: 0, width: 1, height: 1 };
    if (aspectChoice !== null) queueMicrotask(() => chooseAspect(aspectChoice));
  }

  // ── crop interaction ──
  type Handle = 'move' | 'nw' | 'ne' | 'sw' | 'se';
  let drag: { handle: Handle; startX: number; startY: number; start: Crop; rect: DOMRect } | null = null;

  function startDrag(e: PointerEvent, handle: Handle) {
    if (!frame) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { handle, startX: e.clientX, startY: e.clientY, start: { ...crop }, rect: frame.getBoundingClientRect() };
  }

  function moveDrag(e: PointerEvent) {
    if (!drag) return;
    const dx = (e.clientX - drag.startX) / drag.rect.width;
    const dy = (e.clientY - drag.startY) / drag.rect.height;
    const s = drag.start;
    const min = 0.05;
    let { x, y, width, height } = s;
    if (drag.handle === 'move') {
      x = Math.min(Math.max(0, s.x + dx), 1 - s.width);
      y = Math.min(Math.max(0, s.y + dy), 1 - s.height);
    } else {
      const left = drag.handle === 'nw' || drag.handle === 'sw';
      const top = drag.handle === 'nw' || drag.handle === 'ne';
      let x1 = s.x;
      let y1 = s.y;
      let x2 = s.x + s.width;
      let y2 = s.y + s.height;
      if (left) x1 = Math.min(Math.max(0, s.x + dx), x2 - min);
      else x2 = Math.max(Math.min(1, x2 + dx), x1 + min);
      if (top) y1 = Math.min(Math.max(0, s.y + dy), y2 - min);
      else y2 = Math.max(Math.min(1, y2 + dy), y1 + min);
      width = x2 - x1;
      height = y2 - y1;
      if (aspect) {
        // keep pixel aspect: height = width * srcW / (srcH * aspect)
        height = (width * srcW) / (srcH * aspect);
        if (top) y1 = y2 - height;
        if (y1 < 0 || y1 + height > 1) {
          height = top ? y2 : 1 - y1;
          if (top) y1 = y2 - height;
          width = (height * srcH * aspect) / srcW;
          if (left) x1 = x2 - width;
        }
      }
      x = x1;
      y = y1;
    }
    crop = { x, y, width, height };
  }

  function endDrag() {
    drag = null;
  }

  function buildEdits(): EditsDto {
    const full = crop.x <= 0.001 && crop.y <= 0.001 && crop.width >= 0.999 && crop.height >= 0.999;
    const r = (n: number) => Math.round(n * 10000) / 10000;
    return {
      crop: full ? null : { x: r(crop.x), y: r(crop.y), width: r(crop.width), height: r(crop.height) },
      rotate,
      flipHorizontal: flipH,
      flipVertical: flipV,
      brightness: Math.round(brightness),
      contrast: Math.round(contrast),
      saturation: Math.round(saturation),
      filter,
    };
  }

  async function save() {
    busy = true;
    try {
      const res = await api.putEdits(asset.id, buildEdits(), saveAsCopy);
      toast(t('editsSaved'), 'success');
      onsaved(res, saveAsCopy);
    } catch (e) {
      toastError(e);
    } finally {
      busy = false;
    }
  }

  async function revert() {
    if (!(await confirmDialog(t('revertEdits'), { message: t('revertConfirm'), danger: true }))) return;
    busy = true;
    try {
      const res = await api.revertEdits(asset.id);
      toast(t('editsReverted'), 'success');
      onsaved(res, false);
    } catch (e) {
      toastError(e);
    } finally {
      busy = false;
    }
  }

  function resetAll() {
    rotate = 0;
    flipH = flipV = false;
    brightness = contrast = saturation = 0;
    filter = null;
    crop = { x: 0, y: 0, width: 1, height: 1 };
    aspectChoice = null;
    aspect = null;
  }

  function onkey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') onclose();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="editor">
  <header>
    <button class="icon-btn" onclick={onclose} aria-label={t('close')}><Icon name="close" /></button>
    <h2>{t('editorTitle')}</h2>
    <span class="spacer"></span>
    <button class="btn text small" onclick={resetAll}>{t('reset')}</button>
    {#if asset.hasEdits}
      <button class="btn text small" onclick={revert} disabled={busy}>{t('revertEdits')}</button>
    {/if}
    <label class="check small"><input type="checkbox" bind:checked={saveAsCopy} /> {t('saveAsCopy')}</label>
    <button class="btn primary" onclick={save} disabled={busy || !img}>{t('save')}</button>
  </header>

  <div class="stage" bind:this={stage} bind:clientWidth={stageW} bind:clientHeight={stageH}>
    {#if !img}
      <div class="spinner"></div>
    {/if}
    <div class="frame" bind:this={frame} class:hidden={!img}>
      <canvas bind:this={canvas} style="filter: {cssFilter}"></canvas>
      {#if tab === 'crop'}
        <div
          class="crop"
          role="presentation"
          style="left:{crop.x * 100}%;top:{crop.y * 100}%;width:{crop.width * 100}%;height:{crop.height * 100}%"
          onpointerdown={(e) => startDrag(e, 'move')}
          onpointermove={moveDrag}
          onpointerup={endDrag}
        >
          <div class="grid"></div>
          {#each ['nw', 'ne', 'sw', 'se'] as const as h (h)}
            <div
              class="handle {h}"
              role="presentation"
              onpointerdown={(e) => startDrag(e, h)}
              onpointermove={moveDrag}
              onpointerup={endDrag}
            ></div>
          {/each}
        </div>
      {:else if crop.width < 1 || crop.height < 1}
        <div
          class="crop passive"
          style="left:{crop.x * 100}%;top:{crop.y * 100}%;width:{crop.width * 100}%;height:{crop.height * 100}%"
        ></div>
      {/if}
    </div>
    {#if hint}<div class="hint">{t('editorSourceHint')}</div>{/if}
  </div>

  <footer>
    <div class="tabs">
      <button class:active={tab === 'crop'} onclick={() => (tab = 'crop')}><Icon name="crop" size={18} /> {t('crop')}</button>
      <button class:active={tab === 'adjust'} onclick={() => (tab = 'adjust')}><Icon name="tune" size={18} /> {t('adjust')}</button>
      <button class:active={tab === 'filters'} onclick={() => (tab = 'filters')}>{t('filters')}</button>
    </div>
    {#if tab === 'crop'}
      <div class="tools">
        <button class="icon-btn" onclick={doRotate} title={t('rotate')} aria-label={t('rotate')}><Icon name="rotate" /></button>
        <button class="icon-btn" class:active={flipH} onclick={() => (flipH = !flipH)} title={t('flipH')} aria-label={t('flipH')}
          ><Icon name="flip" /></button
        >
        <button
          class="icon-btn"
          class:active={flipV}
          onclick={() => (flipV = !flipV)}
          title={t('flipV')}
          aria-label={t('flipV')}
          style="transform: rotate(90deg)"><Icon name="flip" /></button
        >
        <span class="sep"></span>
        {#each ASPECTS as a (a.label + (a.key ?? ''))}
          <button class="chip" class:active={aspectChoice === a.value} onclick={() => chooseAspect(a.value)}
            >{a.key ? t(a.key) : a.label}</button
          >
        {/each}
      </div>
    {:else if tab === 'adjust'}
      <div class="sliders">
        <label>{t('brightness')} <span>{Math.round(brightness)}</span><input type="range" min="-100" max="100" bind:value={brightness} /></label>
        <label>{t('contrast')} <span>{Math.round(contrast)}</span><input type="range" min="-100" max="100" bind:value={contrast} /></label>
        <label>{t('saturation')} <span>{Math.round(saturation)}</span><input type="range" min="-100" max="100" bind:value={saturation} /></label>
      </div>
    {:else}
      <div class="filters">
        {#each FILTERS as f (f.label)}
          <button class="filter" class:active={filter === f.id} onclick={() => (filter = f.id)}>
            <img src={thumbnailUrl(asset.id)} alt="" style="filter: {f.css}" />
            <span>{t(f.label)}</span>
          </button>
        {/each}
      </div>
    {/if}
  </footer>
</div>

<style>
  .editor {
    position: absolute;
    inset: 0;
    background: #111;
    color: #eee;
    z-index: 20;
    display: flex;
    flex-direction: column;
    --fg: #eee;
    --fg-muted: #aaa;
    --border: #444;
    --bg-hover: rgba(255, 255, 255, 0.1);
    --bg: #222;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  .stage {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    touch-action: none;
  }
  .frame {
    position: relative;
    line-height: 0;
    overflow: hidden;
  }
  .frame.hidden {
    visibility: hidden;
    position: absolute;
  }
  canvas {
    display: block;
  }
  .crop {
    position: absolute;
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.55);
    border: 1px solid rgba(255, 255, 255, 0.9);
    cursor: move;
    touch-action: none;
  }
  .crop.passive {
    cursor: default;
    pointer-events: none;
    border: none;
    box-shadow: 0 0 0 9999px rgba(17, 17, 17, 1);
  }
  .grid {
    position: absolute;
    inset: 0;
    background-image: linear-gradient(to right, rgba(255, 255, 255, 0.35) 1px, transparent 1px),
      linear-gradient(to bottom, rgba(255, 255, 255, 0.35) 1px, transparent 1px);
    background-size: 33.333% 33.333%;
    background-position: -1px -1px;
    pointer-events: none;
  }
  .handle {
    position: absolute;
    width: 22px;
    height: 22px;
    border: 3px solid #fff;
    touch-action: none;
  }
  .handle.nw {
    left: -3px;
    top: -3px;
    border-right: none;
    border-bottom: none;
    cursor: nwse-resize;
  }
  .handle.ne {
    right: -3px;
    top: -3px;
    border-left: none;
    border-bottom: none;
    cursor: nesw-resize;
  }
  .handle.sw {
    left: -3px;
    bottom: -3px;
    border-right: none;
    border-top: none;
    cursor: nesw-resize;
  }
  .handle.se {
    right: -3px;
    bottom: -3px;
    border-left: none;
    border-top: none;
    cursor: nwse-resize;
  }
  .hint {
    position: absolute;
    bottom: 8px;
    left: 50%;
    transform: translateX(-50%);
    font-size: 0.8rem;
    background: rgba(0, 0, 0, 0.6);
    padding: 4px 10px;
    border-radius: 6px;
  }
  footer {
    padding: 0 12px 16px;
    background: #181818;
  }
  .tabs {
    justify-content: center;
    border-bottom-color: #333;
    margin-bottom: 12px;
  }
  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .tools {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .sep {
    width: 1px;
    height: 24px;
    background: #444;
    margin: 0 6px;
  }
  .chip {
    background: #2a2a2a;
    color: #eee;
    border-color: #444;
  }
  .sliders {
    max-width: 560px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .sliders label {
    display: grid;
    grid-template-columns: 110px 40px 1fr;
    align-items: center;
    gap: 8px;
    font-size: 0.9rem;
  }
  .sliders span {
    text-align: right;
    color: #aaa;
  }
  .filters {
    display: flex;
    gap: 10px;
    justify-content: center;
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .filter {
    border: 2px solid transparent;
    background: none;
    border-radius: 10px;
    padding: 2px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    color: #ddd;
    font-size: 0.8rem;
  }
  .filter.active {
    border-color: #8ab4f8;
  }
  .filter img {
    width: 72px;
    height: 72px;
    object-fit: cover;
    border-radius: 8px;
  }
</style>
