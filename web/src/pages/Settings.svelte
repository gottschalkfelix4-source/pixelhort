<script lang="ts">
  import Icon, { type IconName } from '../components/Icon.svelte';
  import Modal from '../components/Modal.svelte';
  import { api, type ApiKeyCreated, type ApiKeyDto, type AuthSessionDto, type DeepPartial, type UserPreferences } from '../lib/api';
  import {
    auth,
    confirmDialog,
    logout,
    prefs,
    promptDialog,
    setTheme,
    toast,
    toastError,
    ui,
    updatePreferences,
    writePref,
    type ThemePref,
  } from '../lib/state.svelte';
  import { navigate } from '../lib/router.svelte';
  import { copyText, formatAbs, formatBytes } from '../lib/utils';
  import { lang, setLang, t, type Lang, type MessageKey } from '../lib/i18n';

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
  let sessions = $state<AuthSessionDto[]>([]);

  $effect(() => {
    api
      .apiKeys()
      .then((k) => (keys = k))
      .catch(() => {});
    api
      .session()
      .then((s) => (hasPin = s.hasPin))
      .catch(() => {});
    api
      .me()
      .then((u) => (auth.user = u))
      .catch(() => {});
    loadSessions();
  });

  function loadSessions() {
    api
      .sessions()
      .then((s) => (sessions = s))
      .catch(() => {});
  }

  const usagePct = $derived(
    auth.user?.quotaBytes ? Math.min(100, (auth.user.usageBytes / auth.user.quotaBytes) * 100) : null,
  );

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
      loadSessions();
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

  async function resetPin() {
    const needsPassword = !!auth.user && (!auth.user.oauthId || auth.status?.passwordLoginEnabled);
    let password: string | undefined;
    if (needsPassword) {
      const v = await promptDialog(t('resetPin'), {
        message: t('resetPinHint'),
        inputType: 'password',
        placeholder: t('password'),
        confirmLabel: t('resetPin'),
      });
      if (v === null) return;
      password = v;
    } else if (!(await confirmDialog(t('resetPin'), { danger: true, confirmLabel: t('resetPin') }))) {
      return;
    }
    try {
      await api.resetPin(password);
      hasPin = false;
      if (auth.user) auth.user.hasPin = false;
      currentPin = newPin = '';
      toast(t('pinReset'), 'success');
    } catch (err) {
      toastError(err);
    }
  }

  async function setPref(patch: DeepPartial<UserPreferences>) {
    try {
      await updatePreferences(patch);
    } catch (err) {
      toastError(err);
    }
  }

  function setLocal(key: 'videoAutoplay' | 'videoLoop' | 'videoMuted', storageKey: string, value: boolean) {
    ui[key] = value;
    writePref(storageKey, String(value));
  }

  function deviceIcon(s: AuthSessionDto): IconName {
    const type = (s.deviceType ?? '').toLowerCase();
    return type === 'ios' || type === 'android' || type === 'mobile' ? 'phone' : 'computer';
  }

  /** Turns a browser user agent into "Chrome on macOS"; other device names are shown as they are. */
  function describeAgent(name: string): string {
    if (!name.startsWith('Mozilla/')) return name;
    const browser = /Edg\//.test(name)
      ? 'Edge'
      : /Firefox\//.test(name)
        ? 'Firefox'
        : /Chrome\//.test(name)
          ? 'Chrome'
          : /Safari\//.test(name)
            ? 'Safari'
            : '';
    const os = /iPhone|iPad/.test(name)
      ? 'iOS'
      : /Android/.test(name)
        ? 'Android'
        : /Mac OS X/.test(name)
          ? 'macOS'
          : /Windows/.test(name)
            ? 'Windows'
            : /Linux/.test(name)
              ? 'Linux'
              : '';
    return [browser, os].filter(Boolean).join(' · ') || name;
  }

  function deviceLabel(s: AuthSessionDto): string {
    const type = s.deviceType ?? '';
    const known: Record<string, string> = { ios: 'iOS', android: 'Android', web: 'Web', mobile: 'Mobile', cli: 'CLI' };
    const parts = [known[type.toLowerCase()] ?? type, s.deviceName ? describeAgent(s.deviceName) : null].filter(Boolean);
    return parts.length ? parts.join(' · ') : t('sessionUnknown');
  }

  async function removeSession(s: AuthSessionDto) {
    try {
      await api.deleteSession(s.id);
      if (s.current) {
        await logout();
        navigate('/login');
        return;
      }
      sessions = sessions.filter((x) => x.id !== s.id);
    } catch (err) {
      toastError(err);
    }
  }

  async function removeOtherSessions() {
    if (
      !(await confirmDialog(t('sessionLogoutOthers'), {
        message: t('sessionLogoutOthersConfirm'),
        danger: true,
        confirmLabel: t('sessionLogoutOthers'),
      }))
    )
      return;
    try {
      await api.deleteOtherSessions();
      loadSessions();
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

  const features: { key: 'memories' | 'people' | 'tags' | 'ratings' | 'sharedLinks'; label: MessageKey }[] = [
    { key: 'memories', label: 'featMemories' },
    { key: 'people', label: 'featPeople' },
    { key: 'tags', label: 'featTags' },
    { key: 'ratings', label: 'featRatings' },
    { key: 'sharedLinks', label: 'featSharedLinks' },
  ];

  const sections: { id: string; label: MessageKey }[] = [
    { id: 'account', label: 'profile' },
    { id: 'appearance', label: 'appearance' },
    { id: 'features', label: 'features' },
    { id: 'security', label: 'lockedTitle' },
    { id: 'sessions', label: 'sessions' },
    { id: 'api-keys', label: 'apiKeys' },
  ];
</script>

<div class="page">
  <div class="page-header"><h1>{t('settings')}</h1></div>
  <nav class="tabs jump">
    {#each sections as s (s.id)}<a href="#{s.id}">{t(s.label)}</a>{/each}
  </nav>
  <div class="sections">
    <section class="card" id="account">
      <h2>{t('profile')}</h2>
      <form class="form" onsubmit={saveProfile}>
        <label class="field">{t('name')}<input type="text" bind:value={name} required /></label>
        <label class="field">{t('email')}<input type="email" bind:value={email} required /></label>
        <div><button class="btn primary">{t('save')}</button></div>
      </form>
      {#if auth.user}
        <div class="storage">
          <h3>{t('storage')}</h3>
          {#if usagePct !== null}
            <div class="bar"><div style="width: {usagePct}%"></div></div>
            <small class="muted">{t('storageOf', { used: formatBytes(auth.user.usageBytes), quota: formatBytes(auth.user.quotaBytes) })}</small>
          {:else}
            <small class="muted">{t('storageUsed', { used: formatBytes(auth.user.usageBytes) })}</small>
          {/if}
        </div>
      {/if}
    </section>

    {#if !auth.user?.oauthId || auth.status?.passwordLoginEnabled}
      <section class="card">
        <h2>{t('changePassword')}</h2>
        <form class="form" onsubmit={savePassword}>
          <label class="field"
            >{t('currentPassword')}<input type="password" bind:value={currentPassword} autocomplete="current-password" /></label
          >
          <label class="field"
            >{t('newPassword')}<input type="password" bind:value={newPassword} autocomplete="new-password" minlength="8" required /></label
          >
          <p class="muted small">{t('passwordChangeHint')}</p>
          <div><button class="btn primary">{t('save')}</button></div>
        </form>
      </section>
    {/if}

    <section class="card" id="appearance">
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
        <h3>{t('playback')}</h3>
        <label class="check"
          ><input
            type="checkbox"
            checked={ui.videoAutoplay}
            onchange={(e) => setLocal('videoAutoplay', 'ph_video_autoplay', e.currentTarget.checked)}
          />
          {t('videoAutoplay')}</label
        >
        <label class="check"
          ><input type="checkbox" checked={ui.videoLoop} onchange={(e) => setLocal('videoLoop', 'ph_video_loop', e.currentTarget.checked)} />
          {t('videoLoop')}</label
        >
        <label class="check"
          ><input type="checkbox" checked={ui.videoMuted} onchange={(e) => setLocal('videoMuted', 'ph_video_muted', e.currentTarget.checked)} />
          {t('videoMuted')}</label
        >
      </div>
    </section>

    <section class="card" id="features">
      <h2>{t('features')}</h2>
      <p class="muted small">{t('featuresHint')}</p>
      <div class="form">
        {#each features as f (f.key)}
          <label class="check"
            ><input type="checkbox" checked={prefs.value[f.key].enabled} onchange={(e) => setPref({ [f.key]: { enabled: e.currentTarget.checked } })} />
            {t(f.label)}</label
          >
        {/each}
        <h3>{t('downloadSettings')}</h3>
        <label class="check"
          ><input
            type="checkbox"
            checked={prefs.value.download.includeLiveVideo}
            onchange={(e) => setPref({ download: { includeLiveVideo: e.currentTarget.checked } })}
          />
          {t('downloadLiveVideo')}</label
        >
      </div>
    </section>

    <section class="card" id="security">
      <h2>{t('lockedTitle')} – {hasPin ? t('changePin') : t('setPin')}</h2>
      <form class="form" onsubmit={savePin}>
        {#if hasPin}
          <label class="field">{t('currentPin')}<input type="password" inputmode="numeric" bind:value={currentPin} autocomplete="off" /></label>
        {/if}
        <label class="field">{t('newPin')}<input type="password" inputmode="numeric" bind:value={newPin} autocomplete="off" /></label>
        <div class="row">
          <button class="btn primary">{t('save')}</button>
          {#if hasPin}<button type="button" class="btn text" onclick={resetPin}>{t('forgotPin')}</button>{/if}
        </div>
      </form>
    </section>

    <section class="card" id="sessions">
      <div class="row">
        <h2>{t('sessions')}</h2>
        <span class="spacer"></span>
        {#if sessions.some((s) => !s.current)}
          <button class="btn" onclick={removeOtherSessions}><Icon name="logout" size={20} /> {t('sessionLogoutOthers')}</button>
        {/if}
      </div>
      <p class="muted small">{t('sessionsHint')}</p>
      <ul class="list">
        {#each sessions as s (s.id)}
          <li class="row">
            <Icon name={deviceIcon(s)} />
            <div class="grow">
              <div>
                {deviceLabel(s)}
                {#if s.current}<span class="badge">{t('sessionCurrent')}</span>{/if}
              </div>
              <small class="muted">{t('sessionLastUsed')}: {formatAbs(s.lastUsedAt)} · {t('createdAt')}: {formatAbs(s.createdAt)}</small>
            </div>
            <button class="icon-btn" onclick={() => removeSession(s)} title={t('sessionLogout')} aria-label={t('sessionLogout')}
              ><Icon name="logout" /></button
            >
          </li>
        {/each}
      </ul>
    </section>

    <section class="card" id="api-keys">
      <div class="row">
        <h2>{t('apiKeys')}</h2>
        <span class="spacer"></span>
        <button class="btn" onclick={createKey}><Icon name="add" size={20} /> {t('newApiKey')}</button>
      </div>
      {#if !keys.length}
        <p class="muted">{t('noApiKeys')}</p>
      {:else}
        <ul class="list">
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
  .jump {
    margin-bottom: 16px;
    flex-wrap: wrap;
  }
  section {
    scroll-margin-top: calc(var(--topbar-h) + 16px);
  }
  section h2 {
    margin-bottom: 14px;
  }
  h3 {
    font-size: 0.95rem;
    font-weight: 500;
    margin: 8px 0 0;
  }
  .storage {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 18px;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--bg-soft);
    overflow: hidden;
  }
  .bar > div {
    height: 100%;
    background: var(--primary);
  }
  .list {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .list li {
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
    gap: 14px;
  }
  .list li:last-child {
    border-bottom: none;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .badge {
    margin-left: 6px;
  }
</style>
