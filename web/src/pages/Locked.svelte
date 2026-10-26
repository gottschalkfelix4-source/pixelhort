<script lang="ts">
  import Timeline from '../components/Timeline.svelte';
  import SelectionBar from '../components/SelectionBar.svelte';
  import Icon from '../components/Icon.svelte';
  import { api, type SessionInfo } from '../lib/api';
  import { Selection } from '../lib/selection.svelte';
  import { auth, toast, toastError } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  const selection = new Selection();
  let session = $state<SessionInfo | null>(null);
  let pin = $state('');
  let pin2 = $state('');
  let error = $state('');
  let busy = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const unlocked = $derived(!!session?.unlockedUntil && new Date(session.unlockedUntil).getTime() > Date.now());

  async function load() {
    try {
      session = await api.session();
      scheduleLock();
    } catch (e) {
      toastError(e);
    }
  }

  function scheduleLock() {
    clearTimeout(timer);
    if (session?.unlockedUntil) {
      const ms = new Date(session.unlockedUntil).getTime() - Date.now();
      if (ms > 0) timer = setTimeout(load, Math.min(ms + 500, 2 ** 31 - 1));
    }
  }

  $effect(() => {
    load();
    return () => clearTimeout(timer);
  });

  const valid = (p: string) => /^\d{4,8}$/.test(p);

  async function setup(e: Event) {
    e.preventDefault();
    error = '';
    if (!valid(pin)) return (error = t('pinInvalid'));
    if (pin !== pin2) return (error = t('pinMismatch'));
    busy = true;
    try {
      await api.setPin({ pin });
      if (auth.user) auth.user.hasPin = true;
      toast(t('pinSaved'), 'success');
      await api.unlockSession(pin);
      pin = pin2 = '';
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function unlock(e: Event) {
    e.preventDefault();
    error = '';
    busy = true;
    try {
      const r = await api.unlockSession(pin);
      pin = '';
      if (session) session.unlockedUntil = r.unlockedUntil;
      scheduleLock();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function lock() {
    try {
      await api.lockSession();
      selection.clear();
      await load();
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{t('lockedTitle')}</h1>
    {#if unlocked}
      <button class="btn" onclick={lock}><Icon name="lock" size={20} /> {t('lockNow')}</button>
    {/if}
  </div>

  {#if !session}
    <div class="spinner"></div>
  {:else if !session.hasPin}
    <form class="card pin-card form" onsubmit={setup}>
      <span class="lock-icon"><Icon name="lock" size={32} /></span>
      <p>{t('lockedSetupText')}</p>
      <label class="field">{t('pin')}<input type="password" inputmode="numeric" bind:value={pin} autocomplete="new-password" /></label>
      <label class="field">{t('pinRepeat')}<input type="password" inputmode="numeric" bind:value={pin2} autocomplete="new-password" /></label>
      {#if error}<p class="error-text">{error}</p>{/if}
      <button class="btn primary" disabled={busy}>{t('setPin')}</button>
    </form>
  {:else if !unlocked}
    <form class="card pin-card form" onsubmit={unlock}>
      <span class="lock-icon"><Icon name="lock" size={32} /></span>
      <p>{t('lockedUnlockText')}</p>
      <!-- svelte-ignore a11y_autofocus -->
      <input type="password" inputmode="numeric" bind:value={pin} autofocus aria-label={t('pin')} autocomplete="off" />
      {#if error}<p class="error-text">{error}</p>{/if}
      <button class="btn primary" disabled={busy}>{t('unlock')}</button>
    </form>
  {:else}
    <Timeline
      filters={{ visibility: 'locked' }}
      {selection}
      emptyText={t('lockedEmpty')}
      emptyHint=""
      emptyIcon="lock"
      viewerOpts={{}}
      onerror={(e) => {
        if (e.status === 403) load();
        else toastError(e);
      }}
    />
  {/if}
</div>
{#if unlocked}
  <SelectionBar {selection} context="locked" />
{/if}

<style>
  .lock-icon {
    width: 72px;
    height: 72px;
    border-radius: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--primary);
    background: var(--primary-soft);
  }
  .pin-card {
    max-width: 400px;
    margin: 48px auto;
    padding: 36px 32px;
    box-shadow: var(--shadow-sm);
    align-items: center;
    text-align: center;
  }
  .pin-card input {
    text-align: center;
    letter-spacing: 0.4em;
    font-size: 1.3rem;
  }
  .pin-card .field {
    width: 100%;
  }
</style>
