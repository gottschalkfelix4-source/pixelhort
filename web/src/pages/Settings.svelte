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

  const showPassword = $derived(!auth.user?.oauthId || !!auth.status?.passwordLoginEnabled);
  const sections = $derived<{ id: string; label: MessageKey; icon: IconName }[]>([
    { id: 'account', label: 'profile', icon: 'person' },
    ...(showPassword ? [{ id: 'password', label: 'changePassword' as MessageKey, icon: 'shield' as IconName }] : []),
    { id: 'appearance', label: 'appearance', icon: 'contrast' },
    { id: 'features', label: 'features', icon: 'tune' },
    { id: 'security', label: 'lockedTitle', icon: 'lock' },
    { id: 'sessions', label: 'sessions', icon: 'computer' },
    { id: 'api-keys', label: 'apiKeys', icon: 'key' },
  ]);
  let current = $state('account');

  // Highlight the section that is currently in view.
  function spy(node: HTMLElement) {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) if (e.isIntersecting) current = (e.target as HTMLElement).id;
      },
      { rootMargin: '-80px 0px -60% 0px' },
    );
    node.querySelectorAll('section[id]').forEach((el) => io.observe(el));
    return { destroy: () => io.disconnect() };
  }

  const themes: { value: ThemePref; label: MessageKey; icon: IconName }[] = [
    { value: 'system', label: 'themeSystem', icon: 'contrast' },
    { value: 'light', label: 'themeLight', icon: 'sun' },
    { value: 'dark', label: 'themeDark', icon: 'moon' },
  ];
</script>

{#snippet head(icon: IconName, title: string, hint: string)}
  <header class="sec-head">
    <span class="sec-icon"><Icon name={icon} size={22} /></span>
    <div>
      <h2>{title}</h2>
      {#if hint}<p>{hint}</p>{/if}
    </div>
  </header>
{/snippet}

<div class="page">
  <div class="page-header"><h1>{t('settings')}</h1></div>
  <div class="layout">
    <nav class="jump" aria-label={t('settings')}>
      {#each sections as s (s.id)}
        <a href="#{s.id}" class:active={current === s.id} onclick={() => (current = s.id)}><Icon name={s.icon} size={20} /> {t(s.label)}</a>
      {/each}
    </nav>
    <div class="sections" use:spy>
      <section class="card" id="account">
        {@render head('person', t('profile'), t('profileHint'))}
        <form class="form" onsubmit={saveProfile}>
          <div class="grid-2">
            <label class="field">{t('name')}<input type="text" bind:value={name} required /></label>
            <label class="field">{t('email')}<input type="email" bind:value={email} required /></label>
          </div>
          <div><button class="btn primary">{t('save')}</button></div>
        </form>
        {#if auth.user}
          <div class="storage">
            <div class="row">
              <Icon name="cloud" size={20} />
              <strong>{t('storage')}</strong>
              <span class="spacer"></span>
              <small class="muted">
                {#if usagePct !== null}
                  {t('storageOf', { used: formatBytes(auth.user.usageBytes), quota: formatBytes(auth.user.quotaBytes) })}
                {:else}
                  {t('storageUsed', { used: formatBytes(auth.user.usageBytes) })}
                {/if}
              </small>
            </div>
            {#if usagePct !== null}
              <div class="bar"><div style="width: {Math.max(2, usagePct)}%"></div></div>
            {/if}
          </div>
        {/if}
      </section>

      {#if showPassword}
        <section class="card" id="password">
          {@render head('shield', t('changePassword'), t('passwordSectionHint'))}
          <form class="form" onsubmit={savePassword}>
            <div class="grid-2">
              <label class="field"
                >{t('currentPassword')}<input type="password" bind:value={currentPassword} autocomplete="current-password" /></label
              >
              <label class="field"
                >{t('newPassword')}<input type="password" bind:value={newPassword} autocomplete="new-password" minlength="8" required /></label
              >
            </div>
            <p class="muted small hint">{t('passwordChangeHint')}</p>
            <div><button class="btn primary">{t('save')}</button></div>
          </form>
        </section>
      {/if}

      <section class="card" id="appearance">
        {@render head('contrast', t('appearance'), t('appearanceHint'))}
        <div class="theme-pick" role="radiogroup" aria-label={t('theme')}>
          {#each themes as th (th.value)}
            <button
              class="theme-opt {th.value}"
              class:active={ui.theme === th.value}
              role="radio"
              aria-checked={ui.theme === th.value}
              onclick={() => setTheme(th.value)}
            >
              <span class="preview" aria-hidden="true"><span></span><span></span><span></span></span>
              <span class="lbl"><Icon name={th.icon} size={18} /> {t(th.label)}</span>
            </button>
          {/each}
        </div>
        <label class="setting">
          <span class="text"><strong>{t('language')}</strong></span>
          <select class="lang" value={lang()} onchange={(e) => setLang((e.currentTarget as HTMLSelectElement).value as Lang)}>
            <option value="de">Deutsch</option>
            <option value="en">English</option>
          </select>
        </label>
        <h3>{t('playback')}</h3>
        <label class="setting">
          <span class="text"><strong>{t('videoAutoplay')}</strong></span>
          <input
            type="checkbox"
            class="switch"
            checked={ui.videoAutoplay}
            onchange={(e) => setLocal('videoAutoplay', 'ph_video_autoplay', e.currentTarget.checked)}
          />
        </label>
        <label class="setting">
          <span class="text"><strong>{t('videoLoop')}</strong></span>
          <input type="checkbox" class="switch" checked={ui.videoLoop} onchange={(e) => setLocal('videoLoop', 'ph_video_loop', e.currentTarget.checked)} />
        </label>
        <label class="setting">
          <span class="text"><strong>{t('videoMuted')}</strong></span>
          <input type="checkbox" class="switch" checked={ui.videoMuted} onchange={(e) => setLocal('videoMuted', 'ph_video_muted', e.currentTarget.checked)} />
        </label>
      </section>

      <section class="card" id="features">
        {@render head('tune', t('features'), t('featuresHint'))}
        {#each features as f (f.key)}
          <label class="setting">
            <span class="text"><strong>{t(f.label)}</strong></span>
            <input
              type="checkbox"
              class="switch"
              checked={prefs.value[f.key].enabled}
              onchange={(e) => setPref({ [f.key]: { enabled: e.currentTarget.checked } })}
            />
          </label>
        {/each}
        <h3>{t('downloadSettings')}</h3>
        <label class="setting">
          <span class="text"><strong>{t('downloadLiveVideo')}</strong></span>
          <input
            type="checkbox"
            class="switch"
            checked={prefs.value.download.includeLiveVideo}
            onchange={(e) => setPref({ download: { includeLiveVideo: e.currentTarget.checked } })}
          />
        </label>
      </section>

      <section class="card" id="security">
        {@render head('lock', `${t('lockedTitle')} – ${hasPin ? t('changePin') : t('setPin')}`, t('pinHint'))}
        <form class="form" onsubmit={savePin}>
          <div class="grid-2">
            {#if hasPin}
              <label class="field">{t('currentPin')}<input type="password" inputmode="numeric" bind:value={currentPin} autocomplete="off" /></label>
            {/if}
            <label class="field">{t('newPin')}<input type="password" inputmode="numeric" bind:value={newPin} autocomplete="off" /></label>
          </div>
          <div class="row">
            <button class="btn primary">{t('save')}</button>
            {#if hasPin}<button type="button" class="btn text" onclick={resetPin}>{t('forgotPin')}</button>{/if}
          </div>
        </form>
      </section>

      <section class="card" id="sessions">
        <div class="row head-row">
          {@render head('computer', t('sessions'), t('sessionsHint'))}
          {#if sessions.some((s) => !s.current)}
            <button class="btn small" onclick={removeOtherSessions}><Icon name="logout" size={18} /> {t('sessionLogoutOthers')}</button>
          {/if}
        </div>
        <ul class="list">
          {#each sessions as s (s.id)}
            <li>
              <span class="list-icon" class:current={s.current}><Icon name={deviceIcon(s)} size={22} /></span>
              <div class="grow">
                <div class="strong">
                  {deviceLabel(s)}
                  {#if s.current}<span class="badge success">{t('sessionCurrent')}</span>{/if}
                </div>
                <small class="muted">{t('sessionLastUsed')}: {formatAbs(s.lastUsedAt)} · {t('createdAt')}: {formatAbs(s.createdAt)}</small>
              </div>
              <button class="icon-btn" onclick={() => removeSession(s)} title={t('sessionLogout')} aria-label={t('sessionLogout')}
                ><Icon name="logout" size={20} /></button
              >
            </li>
          {/each}
        </ul>
      </section>

      <section class="card" id="api-keys">
        <div class="row head-row">
          {@render head('key', t('apiKeys'), t('apiKeysHint'))}
          <button class="btn small" onclick={createKey}><Icon name="add" size={18} /> {t('newApiKey')}</button>
        </div>
        {#if !keys.length}
          <p class="muted small">{t('noApiKeys')}</p>
        {:else}
          <ul class="list">
            {#each keys as k (k.id)}
              <li>
                <span class="list-icon"><Icon name="key" size={22} /></span>
                <div class="grow"><div class="strong">{k.name}</div><small class="muted">{t('createdAt')}: {formatAbs(k.createdAt)}</small></div>
                <button class="icon-btn" onclick={() => deleteKey(k)} title={t('delete')} aria-label={t('delete')}><Icon name="delete" size={20} /></button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    </div>
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
  .layout {
    display: grid;
    grid-template-columns: 220px minmax(0, 760px);
    gap: 32px;
    align-items: start;
  }
  .jump {
    position: sticky;
    top: calc(var(--topbar-h) + 84px);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .jump a {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 14px;
    border-radius: 10px;
    color: var(--fg-muted);
    font-weight: 500;
    font-size: 0.9rem;
    transition:
      background 0.15s,
      color 0.15s;
  }
  .jump a:hover {
    background: var(--bg-hover);
    color: var(--fg);
  }
  .jump a.active {
    background: var(--bg-active);
    color: var(--primary);
    font-weight: 600;
  }
  .sections {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  section {
    scroll-margin-top: calc(var(--topbar-h) + 84px);
  }
  .sec-head {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    margin-bottom: 20px;
    flex: 1;
    min-width: 0;
  }
  .sec-head h2 {
    font-size: 1.05rem;
  }
  .sec-head p {
    margin: 2px 0 0;
    color: var(--fg-muted);
    font-size: 0.85rem;
  }
  .sec-icon {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--primary-soft);
    color: var(--primary);
    flex-shrink: 0;
  }
  .head-row {
    align-items: flex-start;
    flex-wrap: wrap;
  }
  h3 {
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--fg-muted);
    margin: 20px 0 2px;
  }
  .hint {
    margin: -4px 0 0;
  }
  .storage {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 22px;
    padding: 16px;
    border-radius: var(--radius);
    background: var(--bg-soft);
  }
  .storage .row {
    gap: 10px;
  }
  .storage :global(svg) {
    color: var(--primary);
  }
  .bar {
    height: 8px;
    border-radius: 4px;
    background: var(--border);
    overflow: hidden;
  }
  .bar > div {
    height: 100%;
    border-radius: 4px;
    background: var(--primary);
  }
  .theme-pick {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    margin-bottom: 8px;
  }
  .theme-opt {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px;
    border-radius: var(--radius);
    border: 2px solid var(--border);
    background: var(--bg-elev);
    cursor: pointer;
    text-align: left;
    transition: border-color 0.15s;
  }
  .theme-opt:hover {
    border-color: var(--border-strong);
  }
  .theme-opt.active {
    border-color: var(--primary);
  }
  .preview {
    height: 64px;
    border-radius: 8px;
    display: grid;
    grid-template-columns: 28% 1fr;
    grid-template-rows: 14px 1fr;
    gap: 4px;
    padding: 6px;
    background: #f3f4f6;
    border: 1px solid rgba(0, 0, 0, 0.06);
  }
  .preview span {
    border-radius: 3px;
    background: #ffffff;
  }
  .preview span:first-child {
    grid-column: 1 / 3;
  }
  .preview span:last-child {
    background: #c9d8f8;
  }
  .theme-opt.dark .preview {
    background: #0b0b0d;
    border-color: rgba(255, 255, 255, 0.08);
  }
  .theme-opt.dark .preview span {
    background: #1f1f23;
  }
  .theme-opt.dark .preview span:last-child {
    background: #2a3a5c;
  }
  .theme-opt.system .preview {
    background: linear-gradient(90deg, #f3f4f6 50%, #0b0b0d 50%);
    border-color: var(--border);
  }
  .theme-opt.system .preview span {
    background: rgba(128, 128, 128, 0.35);
  }
  .theme-opt.system .preview span:last-child {
    background: rgba(90, 135, 230, 0.55);
  }
  .lbl {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    font-size: 0.88rem;
    padding: 0 2px 2px;
  }
  .lang {
    width: auto;
    min-width: 160px;
  }
  .list-icon.current {
    background: var(--success-soft);
    color: var(--success);
  }
  .strong {
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  @media (max-width: 1100px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
      gap: 16px;
    }
    .jump {
      position: static;
      flex-direction: row;
      overflow-x: auto;
      scrollbar-width: none;
      margin: -4px 0 0;
    }
    .jump a {
      white-space: nowrap;
      padding: 8px 12px;
    }
  }
  @media (max-width: 600px) {
    .theme-pick {
      gap: 8px;
    }
    .preview {
      height: 48px;
    }
  }
</style>
