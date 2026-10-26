<script lang="ts">
  import Modal from './Modal.svelte';
  import Icon from './Icon.svelte';
  import { api, type SharedLinkDto, type Uuid } from '../lib/api';
  import { toast, toastError } from '../lib/state.svelte';
  import { copyText, fromDateTimeLocal, toDateTimeLocal } from '../lib/utils';
  import { t } from '../lib/i18n';

  let {
    link = null,
    assetIds = [],
    albumId = null,
    onclose,
    onsaved,
  }: {
    /** existing link to edit; otherwise a new link is created */
    link?: SharedLinkDto | null;
    assetIds?: Uuid[];
    albumId?: Uuid | null;
    onclose: () => void;
    onsaved?: (l: SharedLinkDto) => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  const initial = link;
  let description = $state(initial?.description ?? '');
  let expires = $state(toDateTimeLocal(initial?.expiresAt));
  let password = $state('');
  let removePassword = $state(false);
  let allowDownload = $state(initial?.allowDownload ?? true);
  let allowUpload = $state(initial?.allowUpload ?? false);
  let showMetadata = $state(initial?.showMetadata ?? true);
  let busy = $state(false);
  let created = $state<SharedLinkDto | null>(null);

  const isAlbum = $derived(initial ? initial.type === 'ALBUM' : !!albumId);

  async function submit(e: Event) {
    e.preventDefault();
    busy = true;
    try {
      let res: SharedLinkDto;
      if (initial) {
        const patch: Parameters<typeof api.updateSharedLink>[1] = {
          description,
          expiresAt: fromDateTimeLocal(expires),
          allowDownload,
          allowUpload,
          showMetadata,
        };
        if (removePassword) patch.password = '';
        else if (password) patch.password = password;
        res = await api.updateSharedLink(initial.id, patch);
        toast(t('saved'), 'success');
        onsaved?.(res);
        onclose();
      } else {
        res = await api.createSharedLink({
          type: albumId ? 'ALBUM' : 'INDIVIDUAL',
          albumId: albumId ?? undefined,
          assetIds: albumId ? undefined : assetIds,
          description: description || undefined,
          password: password || undefined,
          expiresAt: fromDateTimeLocal(expires),
          allowDownload,
          allowUpload,
          showMetadata,
        });
        created = res;
        await copyText(res.url);
        toast(t('linkCreated'), 'success');
        onsaved?.(res);
      }
    } catch (err) {
      toastError(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={initial ? t('editLink') : t('createSharedLink')} {onclose} width={480}>
  {#if created}
    <div class="form">
      <div class="url-row">
        <input type="text" readonly value={created.url} onfocus={(e) => (e.currentTarget as HTMLInputElement).select()} />
        <button
          class="icon-btn"
          aria-label={t('copy')}
          onclick={async () => {
            await copyText(created!.url);
            toast(t('copied'));
          }}><Icon name="copy" /></button
        >
      </div>
      <div class="row"><span class="spacer"></span><button class="btn primary" onclick={onclose}>{t('done')}</button></div>
    </div>
  {:else}
    <form class="form" id="share-link-form" onsubmit={submit}>
      {#if !initial && !albumId}
        <p class="muted">{t('linkIndividual', { n: assetIds.length })}</p>
      {/if}
      <label class="field">{t('description')}<input type="text" bind:value={description} /></label>
      <label class="field">{t('expiresAt')} ({t('optional')})<input type="datetime-local" bind:value={expires} /></label>
      <label class="field"
        >{t('linkPassword')}<input type="password" bind:value={password} autocomplete="new-password" disabled={removePassword} /></label
      >
      {#if initial?.hasPassword}
        <label class="check"><input type="checkbox" bind:checked={removePassword} /> {t('removePassword')}</label>
      {/if}
      <label class="check"><input type="checkbox" bind:checked={allowDownload} /> {t('allowDownload')}</label>
      {#if isAlbum}
        <label class="check"><input type="checkbox" bind:checked={allowUpload} /> {t('allowUpload')}</label>
      {/if}
      <label class="check"><input type="checkbox" bind:checked={showMetadata} /> {t('showMetadata')}</label>
      <div class="row">
        <span class="spacer"></span>
        <button type="button" class="btn text" onclick={onclose}>{t('cancel')}</button>
        <button type="submit" class="btn primary" disabled={busy}>{initial ? t('save') : t('create')}</button>
      </div>
    </form>
  {/if}
</Modal>

<style>
  .url-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
</style>
