<script lang="ts">
  import Icon from './Icon.svelte';
  import MiniMap from './MiniMap.svelte';
  import Pickers from './Pickers.svelte';
  import { api, personThumbnailUrl, thumbnailUrl, type AssetDto, type FaceDto, type ShareAccess } from '../lib/api';
  import { auth, prefs, promptDialog, toast, toastError } from '../lib/state.svelte';
  import { formatAbs, formatBytes, formatLocal } from '../lib/utils';
  import { t } from '../lib/i18n';

  let {
    asset,
    share = null,
    readOnly = false,
    onupdate,
    onclose,
  }: {
    asset: AssetDto;
    share?: ShareAccess | null;
    readOnly?: boolean;
    onupdate: (a: AssetDto) => void;
    onclose: () => void;
  } = $props();

  const canEdit = $derived(!readOnly && !share && asset.ownerId === auth.user?.id);
  let description = $state('');
  let faces = $state<FaceDto[]>([]);
  let reassigning = $state<FaceDto | null>(null);
  let tagPicker = $state(false);

  $effect(() => {
    description = asset.description ?? '';
  });

  $effect(() => {
    const id = asset.id;
    faces = [];
    if (share || readOnly) return;
    api
      .faces(id)
      .then((f) => {
        if (asset.id === id) faces = f;
      })
      .catch(() => {});
  });

  async function update(patch: Parameters<typeof api.updateAsset>[1]) {
    try {
      onupdate(await api.updateAsset(asset.id, patch));
    } catch (e) {
      toastError(e);
    }
  }

  function saveDescription() {
    if (description !== (asset.description ?? '')) update({ description });
  }

  function setRating(r: number) {
    update({ rating: asset.rating === r ? null : r });
  }

  async function editDate() {
    const cur = asset.localDateTime?.slice(0, 16) ?? '';
    const v = await promptDialog(t('dateTaken'), { value: cur, inputType: 'datetime-local' });
    if (v) update({ dateTimeOriginal: `${v.length === 16 ? v + ':00' : v}Z` });
  }

  async function setCover(f: FaceDto) {
    if (!f.person) return;
    try {
      await api.updatePerson(f.person.id, { featureFaceId: f.id });
      toast(t('coverUpdated'), 'success');
    } catch (e) {
      toastError(e);
    }
  }

  async function editLocation() {
    const cur = exif?.latitude != null && exif?.longitude != null ? `${exif.latitude}, ${exif.longitude}` : '';
    const v = await promptDialog(t('setLocation'), { value: cur, placeholder: t('locationPrompt') });
    if (!v?.trim()) return;
    const [lat, lon] = v.split(/[,;\s]+/).filter(Boolean).map(Number);
    if (!Number.isFinite(lat) || !Number.isFinite(lon) || Math.abs(lat) > 90 || Math.abs(lon) > 180)
      return toast(t('invalidLocation'), 'error');
    update({ latitude: lat, longitude: lon });
  }

  async function reloadAsset() {
    try {
      onupdate(await api.asset(asset.id));
    } catch {
      /* ignore */
    }
  }

  async function removeTag(id: string) {
    try {
      await api.untagAssets(id, [asset.id]);
      reloadAsset();
    } catch (e) {
      toastError(e);
    }
  }

  function faceStyle(f: FaceDto) {
    const size = 56;
    const w = Math.max(1, f.boundingBoxX2 - f.boundingBoxX1);
    const h = Math.max(1, f.boundingBoxY2 - f.boundingBoxY1);
    const s = size / Math.max(w, h);
    const url = thumbnailUrl(asset.id, 'preview', share);
    return `background-image:url(${url});background-size:${f.imageWidth * s}px ${f.imageHeight * s}px;background-position:${-f.boundingBoxX1 * s + (size - w * s) / 2}px ${-f.boundingBoxY1 * s + (size - h * s) / 2}px`;
  }

  const exif = $derived(asset.exif);
  const placeLine = $derived([exif?.city, exif?.state, exif?.country].filter(Boolean).join(', '));
  const cameraLine = $derived([exif?.make, exif?.model].filter(Boolean).join(' '));
  const settingsLine = $derived(
    [
      exif?.fNumber ? `ƒ/${exif.fNumber}` : null,
      exif?.exposureTime ? `${exif.exposureTime} s` : null,
      exif?.focalLength ? `${exif.focalLength} mm` : null,
      exif?.iso ? `ISO ${exif.iso}` : null,
    ]
      .filter(Boolean)
      .join(' · '),
  );
  const mp = $derived(asset.width && asset.height ? ((asset.width * asset.height) / 1e6).toFixed(1) + ' MP' : '');
</script>

<aside class="info">
  <header>
    <button class="icon-btn" onclick={onclose} aria-label={t('close')}><Icon name="close" /></button>
    <h2>{t('info')}</h2>
  </header>

  <section>
    {#if canEdit}
      <textarea
        rows="2"
        bind:value={description}
        placeholder={t('addDescription')}
        onblur={saveDescription}
        onkeydown={(e) => e.stopPropagation()}
      ></textarea>
    {:else if asset.description}
      <p>{asset.description}</p>
    {/if}
  </section>

  {#if prefs.value.ratings.enabled && (canEdit || asset.rating)}
    <section class="rating">
      {#each [1, 2, 3, 4, 5] as r (r)}
        <button class="star" disabled={!canEdit} onclick={() => setRating(r)} aria-label="{t('rating')} {r}">
          <Icon name={(asset.rating ?? 0) >= r ? 'star' : 'starOutline'} size={22} />
        </button>
      {/each}
    </section>
  {/if}

  {#if faces.length || asset.people.length}
    <section>
      <h3>{t('people')}</h3>
      <div class="faces">
        {#if faces.length}
          {#each faces as f (f.id)}
            <div class="face">
              <button class="face-img" style={faceStyle(f)} onclick={() => !share && (reassigning = f)} title={t('reassignFace')}
              ></button>
              {#if f.person}
                <a href="/people/{f.person.id}" class="small">{f.person.name || t('unnamedPerson')}</a>
                {#if canEdit}
                  <button class="cover-btn" onclick={() => setCover(f)} title={t('setAsPersonCover')} aria-label={t('setAsPersonCover')}
                    ><Icon name="person" size={14} /></button
                  >
                {/if}
              {:else}
                <span class="small muted">{t('unnamedPerson')}</span>
              {/if}
            </div>
          {/each}
        {:else}
          {#each asset.people as p (p.id)}
            <a class="face" href="/people/{p.id}">
              <img class="face-img" src={personThumbnailUrl(p.id)} alt="" />
              <span class="small">{p.name || t('unnamedPerson')}</span>
            </a>
          {/each}
        {/if}
      </div>
    </section>
  {/if}

  <section>
    <h3>{t('details')}</h3>
    <div class="line">
      <Icon name="calendar" />
      <div>
        <div>{formatLocal(asset.localDateTime, { dateStyle: 'full' })}</div>
        <div class="muted small">
          {formatLocal(asset.localDateTime, { timeStyle: 'short' })}{exif?.timeZone ? ` · ${exif.timeZone}` : ''}
        </div>
      </div>
      {#if canEdit}<button class="icon-btn" onclick={editDate} aria-label={t('edit')}><Icon name="edit" size={18} /></button>{/if}
    </div>
    <div class="line">
      <Icon name={asset.type === 'VIDEO' ? 'video' : 'photo'} />
      <div>
        <div class="break">{asset.originalFileName}</div>
        <div class="muted small">
          {[mp, asset.width && asset.height ? `${asset.width} × ${asset.height}` : '', formatBytes(asset.fileSize)].filter(Boolean).join(' · ')}
        </div>
      </div>
    </div>
    {#if cameraLine || settingsLine}
      <div class="line">
        <Icon name="camera" />
        <div>
          <div>{cameraLine || t('unknown')}</div>
          {#if exif?.lensModel}<div class="muted small">{exif.lensModel}</div>{/if}
          {#if settingsLine}<div class="muted small">{settingsLine}</div>{/if}
        </div>
      </div>
    {/if}
    {#if placeLine || (exif?.latitude != null && exif?.longitude != null) || canEdit}
      <div class="line">
        <Icon name="place" />
        <div class="grow">
          {#if placeLine}<div>{placeLine}</div>{/if}
          {#if canEdit}
            <button class="btn text small loc-btn" onclick={editLocation}><Icon name="edit" size={16} /> {t('setLocation')}</button>
          {/if}
          {#if exif?.latitude != null && exif?.longitude != null && auth.serverInfo?.features.map !== false}
            <MiniMap lat={exif.latitude} lon={exif.longitude} />
          {/if}
        </div>
      </div>
    {/if}
    <div class="muted small uploaded">
      {t('createdAt')}: {formatAbs(asset.createdAt)}
    </div>
  </section>

  {#if !share}
    <section>
      <h3>{t('tags')}</h3>
      <div class="row wrap">
        {#each asset.tags as tag (tag.id)}
          <span class="chip">
            <span class="dot" style="background: {tag.color || 'var(--fg-muted)'}"></span>
            <a href="/tags/{tag.id}">{tag.value}</a>
            {#if canEdit}<button class="x" onclick={() => removeTag(tag.id)} aria-label={t('remove')}><Icon name="close" size={14} /></button>{/if}
          </span>
        {/each}
        {#if canEdit}
          <button class="chip" onclick={() => (tagPicker = true)}><Icon name="add" size={16} /> {t('addTag')}</button>
        {/if}
      </div>
    </section>
  {/if}
</aside>

{#if reassigning}
  <Pickers
    kind="person"
    title={t('reassignFace')}
    onclose={() => (reassigning = null)}
    onpick={async (p) => {
      const face = reassigning!;
      reassigning = null;
      if (p.kind !== 'person') return;
      try {
        const updated = p.person ? await api.reassignFace(face.id, p.person.id) : await api.reassignFace(face.id, null, true);
        faces = faces.map((f) => (f.id === face.id ? updated : f));
        reloadAsset();
      } catch (e) {
        toastError(e);
      }
    }}
  />
{/if}

{#if tagPicker}
  <Pickers
    kind="tag"
    exclude={asset.tags.map((x) => x.id)}
    onclose={() => (tagPicker = false)}
    onpick={async (p) => {
      tagPicker = false;
      if (p.kind !== 'tag') return;
      try {
        await api.tagAssets(p.tag.id, [asset.id]);
        reloadAsset();
      } catch (e) {
        toastError(e);
      }
    }}
  />
{/if}

<style>
  .info {
    width: 360px;
    max-width: 100vw;
    height: 100%;
    overflow-y: auto;
    background: var(--bg-elev);
    color: var(--fg);
    padding: 8px 16px 32px;
    flex-shrink: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  section {
    margin-bottom: 20px;
  }
  h3 {
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--fg-muted);
    margin-bottom: 10px;
  }
  .cover-btn {
    border: none;
    background: none;
    color: var(--fg-muted);
    cursor: pointer;
    padding: 2px;
    line-height: 0;
  }
  .cover-btn:hover {
    color: var(--primary);
  }
  .rating {
    display: flex;
    gap: 2px;
  }
  .star {
    border: none;
    background: none;
    color: var(--warning);
    cursor: pointer;
    padding: 2px;
  }
  .star:disabled {
    cursor: default;
  }
  .line {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    margin-bottom: 14px;
    color: var(--fg);
  }
  .line > :global(svg) {
    color: var(--fg-muted);
    margin-top: 2px;
  }
  .line > div {
    flex: 1;
    min-width: 0;
  }
  .grow {
    flex: 1;
  }
  .break {
    word-break: break-all;
  }
  .faces {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .face {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 64px;
    text-align: center;
    color: var(--fg);
  }
  .face-img {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    border: none;
    background-color: var(--bg-soft);
    background-repeat: no-repeat;
    object-fit: cover;
    cursor: pointer;
    padding: 0;
  }
  .chip a {
    color: inherit;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .x {
    border: none;
    background: none;
    padding: 0;
    display: flex;
    cursor: pointer;
    color: var(--fg-muted);
  }
  .uploaded {
    margin-top: 8px;
  }
  @media (max-width: 700px) {
    .info {
      width: 100vw;
    }
  }
</style>
