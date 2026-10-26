<script lang="ts">
  import { api } from '../lib/api';
  import { auth, initAuth, loggedIn } from '../lib/state.svelte';
  import { navigate, router } from '../lib/router.svelte';
  import { t } from '../lib/i18n';
  import Logo from '../components/Logo.svelte';

  let email = $state('');
  let password = $state('');
  let name = $state('');
  let error = $state('');
  let busy = $state(false);

  const setup = $derived(!!auth.status?.setupRequired);
  const redirect = $derived.by(() => {
    const r = router.query.get('redirect');
    return r && r.startsWith('/') && !r.startsWith('//') ? r : '/photos';
  });

  $effect(() => {
    if (auth.user) navigate(redirect, { replace: true });
  });

  async function submit(e: Event) {
    e.preventDefault();
    error = '';
    busy = true;
    try {
      const res = setup
        ? await api.setup({ email, password, name })
        : await api.login({ email, password, deviceType: 'web', deviceName: navigator.userAgent.slice(0, 80) });
      loggedIn(res.accessToken, res.user);
    } catch (err) {
      error = err instanceof Error ? err.message : t('loginFailed');
    } finally {
      busy = false;
    }
  }

  async function oauth() {
    error = '';
    try {
      const redirectUri = `${location.origin}/auth/oauth-callback`;
      sessionStorage.setItem('ph_oauth_redirect', redirect);
      const { url } = await api.oauthAuthorize(redirectUri);
      location.href = url;
    } catch (err) {
      error = err instanceof Error ? err.message : t('oauthFailed');
    }
  }
</script>

<div class="wrap">
  <div class="glow" aria-hidden="true"></div>
  <div class="card login">
    <div class="brand">
      <Logo size={64} />
      <h1>{setup ? t('setupTitle') : t('appName')}</h1>
      {#if setup}<p class="muted">{t('setupText')}</p>{:else}<p class="muted">{t('loginTitle')}</p>{/if}
    </div>
    {#if !setup && auth.status?.loginPageMessage}
      <p class="login-message">{auth.status.loginPageMessage}</p>
    {/if}

    {#if !auth.status}
      <p class="error-text">{t('serverUnreachable')}</p>
      <button class="btn" onclick={() => initAuth()}>{t('retry')}</button>
    {:else}
      {#if setup || auth.status.passwordLoginEnabled}
        <form class="form" onsubmit={submit}>
          {#if setup}
            <label class="field">{t('name')}<input type="text" bind:value={name} required autocomplete="name" /></label>
          {/if}
          <label class="field">{t('email')}<input type="email" bind:value={email} required autocomplete="username" /></label>
          <label class="field"
            >{t('password')}<input
              type="password"
              bind:value={password}
              required
              autocomplete={setup ? 'new-password' : 'current-password'}
            /></label
          >
          {#if error}<p class="error-text error-box">{error}</p>{/if}
          <button class="btn primary submit" type="submit" disabled={busy}>{setup ? t('setupButton') : t('loginButton')}</button>
        </form>
      {:else}
        <p class="muted">{t('passwordLoginDisabled')}</p>
        {#if error}<p class="error-text error-box">{error}</p>{/if}
      {/if}

      {#if !setup && auth.status.oauthEnabled}
        {#if auth.status.passwordLoginEnabled}<div class="or"><span>{t('or')}</span></div>{/if}
        <button class="btn oauth" onclick={oauth}>{auth.status.oauthButtonText || 'SSO'}</button>
      {/if}
    {/if}
  </div>
</div>

<style>
  .login-message {
    white-space: pre-line;
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--primary-soft);
    color: var(--fg);
    margin: 0;
    font-size: 0.9rem;
  }
  .wrap {
    position: relative;
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
    background: var(--bg);
    overflow: hidden;
  }
  .glow {
    position: absolute;
    inset: -20%;
    background:
      radial-gradient(40% 35% at 25% 25%, rgba(59, 140, 255, 0.28), transparent 70%),
      radial-gradient(35% 30% at 78% 70%, rgba(255, 196, 46, 0.18), transparent 70%),
      radial-gradient(45% 40% at 70% 15%, rgba(21, 87, 201, 0.22), transparent 70%);
    filter: blur(20px);
    pointer-events: none;
  }
  .login {
    position: relative;
    width: 100%;
    max-width: 420px;
    padding: 40px 36px 36px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    box-shadow: var(--shadow);
    border-radius: 28px;
    background: color-mix(in srgb, var(--bg-elev) 88%, transparent);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
  }
  .brand {
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }
  .brand h1 {
    margin-top: 10px;
    font-size: 1.6rem;
  }
  .brand p {
    margin: 0;
  }
  .submit {
    height: 46px;
    margin-top: 4px;
    font-size: 0.95rem;
  }
  .error-box {
    margin: 0;
    padding: 10px 14px;
    border-radius: 10px;
    background: var(--danger-soft);
    font-size: 0.88rem;
  }
  .or {
    text-align: center;
    color: var(--fg-muted);
    position: relative;
    font-size: 0.85rem;
  }
  .or::before {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    border-top: 1px solid var(--border);
  }
  .or span {
    background: var(--bg-elev);
    padding: 0 12px;
    position: relative;
  }
  .oauth {
    width: 100%;
    height: 46px;
  }
  @media (max-width: 480px) {
    .login {
      padding: 32px 22px 26px;
    }
  }
</style>
