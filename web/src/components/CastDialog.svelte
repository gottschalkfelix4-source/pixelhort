<script lang="ts">
  import Modal from './Modal.svelte';
  import Icon from './Icon.svelte';
  import { api, type AssetDto, type CastDevice } from '../lib/api';
  import { castAsset, CastUnavailableError } from '../lib/cast';
  import { toast, toastError } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  let { asset, onclose }: { asset: AssetDto; onclose: () => void } = $props();

  let devices = $state<CastDevice[]>([]);
  let searching = $state(false);
  let busy = $state<string | null>(null);
  let playingOn = $state<string | null>(null);
  let address = $state('');

  async function search(refresh: boolean) {
    searching = true;
    try {
      devices = await api.castDevices(refresh);
    } catch (e) {
      toastError(e);
    } finally {
      searching = false;
    }
  }

  $effect(() => {
    search(false);
  });

  async function playOn(device: CastDevice) {
    busy = device.id;
    try {
      await api.castPlay(device.id, asset.id);
      playingOn = device.id;
      toast(t('castStarted', { device: device.name }), 'success');
    } catch (e) {
      toastError(e);
    } finally {
      busy = null;
    }
  }

  async function stop(device: CastDevice) {
    try {
      await api.castStop(device.id);
      playingOn = null;
      toast(t('castStopped'));
    } catch (e) {
      toastError(e);
    }
  }

  async function googleCast() {
    try {
      const device = await castAsset(asset);
      toast(t('castStarted', { device }), 'success');
      onclose();
    } catch (e) {
      if (e instanceof CastUnavailableError) toast(t('castUnavailable'), 'error', 5000);
      else if (e && typeof e === 'object' && 'code' in e && (e as { code: string }).code === 'cancel') return;
      else toast(t('castFailed'), 'error');
    }
  }

  async function addDevice(e: SubmitEvent) {
    e.preventDefault();
    if (!address.trim()) return;
    try {
      const device = await api.castAddDevice(address.trim());
      devices = [...devices.filter((d) => d.id !== device.id), device];
      address = '';
    } catch (err) {
      toastError(err);
    }
  }
</script>

<Modal title={t('castChooseDevice')} {onclose} width={440}>
  <button class="row google" onclick={googleCast}>
    <Icon name="cast" />
    <span>{t('castGoogle')}</span>
  </button>

  <div class="section">
    <span>{t('castTvs')}</span>
    <button class="link" onclick={() => search(true)} disabled={searching}>{t('castRefresh')}</button>
  </div>

  {#if searching}
    <p class="muted">{t('castSearching')}</p>
  {:else if devices.length === 0}
    <p class="muted">{t('castNoDevices')}</p>
  {/if}

  {#each devices as device (device.id)}
    <div class="row">
      <button class="device" onclick={() => playOn(device)} disabled={busy !== null}>
        <Icon name="cast" />
        <span class="name">
          {device.name}
          {#if device.model || device.manufacturer}
            <small>{[device.manufacturer, device.model].filter(Boolean).join(' · ')}</small>
          {/if}
        </span>
        {#if busy === device.id}<span class="muted">…</span>{/if}
      </button>
      {#if playingOn === device.id}
        <button class="link" onclick={() => stop(device)}>{t('castStop')}</button>
      {/if}
    </div>
  {/each}

  <form class="add" onsubmit={addDevice}>
    <label for="cast-address">{t('castAddManual')}</label>
    <div class="inline">
      <input id="cast-address" type="url" placeholder={t('castAddPlaceholder')} bind:value={address} />
      <button type="submit" class="btn">{t('castAdd')}</button>
    </div>
  </form>
</Modal>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }
  .row.google,
  .device {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    padding: 10px 12px;
    border-radius: 10px;
    background: transparent;
    color: inherit;
    border: 1px solid var(--border, rgba(127, 127, 127, 0.25));
    text-align: left;
    cursor: pointer;
    margin-bottom: 8px;
  }
  .row.google:hover,
  .device:hover:not(:disabled) {
    background: var(--hover, rgba(127, 127, 127, 0.12));
  }
  .name {
    display: flex;
    flex-direction: column;
    flex: 1;
  }
  small,
  .muted {
    opacity: 0.65;
    font-size: 0.85em;
  }
  .section {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin: 14px 0 8px;
    font-weight: 600;
  }
  .link {
    background: none;
    border: none;
    color: var(--accent, #1a73e8);
    cursor: pointer;
    padding: 4px;
  }
  .add {
    margin-top: 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .inline {
    display: flex;
    gap: 8px;
  }
  .inline input {
    flex: 1;
    min-width: 0;
  }
</style>
