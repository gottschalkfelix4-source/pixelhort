<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import Modal from '../components/Modal.svelte';
  import { api, type ApiKeyCreated, type ApiKeyDto } from '../lib/api';
  import { auth, confirmDialog, promptDialog, setTheme, toast, toastError, ui, type ThemePref } from '../lib/state.svelte';
  import { copyText, formatAbs } from '../lib/utils';
  import { lang, setLang, t, type Lang } from '../lib/i18n';

  // svelte-ignore state_referenced_locally
  let name = $state(auth.user?.name ?? '');
  // svelte-ignore state_referenced_locally
  let email = $state(auth.user?.email ?? '');
  let currentPassword = $state('');
  let newPassword = $state('');
  let keys = $state<ApiKeyDto[]>([]);
  let created = $state<ApiKeyCreated | null>(null);
  let currentPin = $state('');
  let newPin = $state('');
  let hasPin = $state(auth.user?.hasPin ?? false);

  $effect(() => {
    api
      .apiKeys()
      .then((k) => (keys = k))
      .catch(() => {});
    api
      .session()
      .then((s) => (hasPin = s.hasPin))
      .catch(() => {});
  });

  async function saveProfile(e: Event) {
    e.preventDefault();
    try {
      auth.user = await api.updateMe({ name, email });
      toast(t('saved'), 'success');
    } catch (err) {
      toastError(err);
    }
  }

  async function savePassword(e: Event) {
    e.preventDefault();
    try {
      auth.user = await api.updateMe({ password: newPassword, currentPassword });
      currentPassword = newPassword = '';
      toast(t('passwordChanged'), 'success');
    } catch (err) {
      toastError(err);
    }
  }

  async function savePin(e: Event) {
    e.preventDefault();
    if (!/^\d{4,8}$/.test(newPin)) return toast(t('pinInvalid'), 'error');
    try {
      await api.setPin({ pin: newPin, currentPin: hasPin ? currentPin : undefined });
      hasPin = true;
      if (auth.user) auth.user.hasPin = true;
      currentPin = newPin = '';
      toast(t('pinSaved'), 'success');
    } catch (err) {
      toastError(err);
    }
  }

  async function createKey() {
    const n = await promptDialog(t('newApiKey'), { placeholder: t('name'), confirmLabel: t('create') });
    if (!n?.trim()) return;
    try {
      created = await api.createApiKey(n.trim());
      keys = [...keys, { id: created.id, name: created.name, createdAt: created.createdAt }];
    } catch (e) {
      toastError(e);
    }
  }

  async function deleteKey(k: ApiKeyDto) {
    if (!(await confirmDialog(t('delete'), { message: t('deleteApiKeyConfirm', { name: k.name }), danger: true, confirmLabel: t('delete') })))
      return;
    try {
      await api.deleteApiKey(k.id);
      keys = keys.filter((x) => x.id !== k.id);
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="page">
  <div class="page-header"><h1>{t('settings')}</h1></div>
  <div class="sections">
    <section class="card">
      <h2>{t('profile')}</h2>
      <form class="form" onsubmit={saveProfile}>
        <label class="field">{t('name')}<input type="text" bind:value={name} /></label>
        <label class="field">{t('email')}<input type="email" bind:value={email} /></label>
        <div><button class="btn primary">{t('save')}</button></div>
      </form>
    </section>

    {#if !auth.user?.oauthId || auth.status?.passwordLoginEnabled}
      <section class="card">
        <h2>{t('changePassword')}</h2>
        <form class="form" onsubmit={savePassword}>
          <label class="field"
            >{t('currentPassword')}<input type="password" bind:value={currentPassword} autocomplete="current-password" /></label
          >
          <label class="field">{t('newPassword')}<input type="password" bind:value={newPassword} autocomplete="new-password" required /></label>
          <div><button class="btn primary">{t('save')}</button></div>
        </form>
      </section>
    {/if}

    <section class="card">
      <h2>{t('appearance')}</h2>
      <div class="form">
        <label class="field"
          >{t('theme')}
          <select value={ui.theme} onchange={(e) => setTheme((e.currentTarget as HTMLSelectElement).value as ThemePref)}>
            <option value="system">{t('themeSystem')}</option>
            <option value="light">{t('themeLight')}</option>
            <option value="dark">{t('themeDark')}</option>
          </select>
        </label>
        <label class="field"
          >{t('language')}
          <select value={lang()} onchange={(e) => setLang((e.currentTarget as HTMLSelectElement).value as Lang)}>
            <option value="de">Deutsch</option>
            <option value="en">English</option>
          </select>
        </label>
      </div>
    </section>

    <section class="card">
      <h2>{t('lockedTitle')} – {hasPin ? t('changePin') : t('setPin')}</h2>
      <form class="form" onsubmit={savePin}>
        {#if hasPin}
          <label class="field">{t('currentPin')}<input type="password" inputmode="numeric" bind:value={currentPin} autocomplete="off" /></label>
        {/if}
        <label class="field">{t('newPin')}<input type="password" inputmode="numeric" bind:value={newPin} autocomplete="off" /></label>
        <div><button class="btn primary">{t('save')}</button></div>
      </form>
    </section>

    <section class="card">
      <div class="row">
        <h2>{t('apiKeys')}</h2>
        <span class="spacer"></span>
        <button class="btn" onclick={createKey}><Icon name="add" size={20} /> {t('newApiKey')}</button>
      </div>
      {#if !keys.length}
        <p class="muted">{t('noApiKeys')}</p>
      {:else}
        <ul class="keys">
          {#each keys as k (k.id)}
            <li class="row">
              <Icon name="key" />
              <div class="grow"><div>{k.name}</div><small class="muted">{t('createdAt')}: {formatAbs(k.createdAt)}</small></div>
              <button class="icon-btn" onclick={() => deleteKey(k)} aria-label={t('delete')}><Icon name="delete" /></button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>
</div>

{#if created}
  <Modal title={t('newApiKey')} onclose={() => (created = null)}>
    <p class="muted">{t('apiKeySecretHint')}</p>
    <div class="row">
      <input type="text" readonly value={created.secret} onfocus={(e) => (e.currentTarget as HTMLInputElement).select()} />
      <button
        class="icon-btn"
        aria-label={t('copy')}
        onclick={async () => {
          await copyText(created!.secret);
          toast(t('copied'));
        }}><Icon name="copy" /></button
      >
    </div>
    {#snippet footer()}
      <button class="btn primary" onclick={() => (created = null)}>{t('done')}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .sections {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 720px;
  }
  section h2 {
    margin-bottom: 14px;
  }
  .keys {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .keys li {
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
    gap: 14px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
</style>
