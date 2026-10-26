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
          {#if error}<p class="error-text">{error}</p>{/if}
          <button class="btn primary" type="submit" disabled={busy}>{setup ? t('setupButton') : t('loginButton')}</button>
        </form>
      {:else}
        <p class="muted">{t('passwordLoginDisabled')}</p>
        {#if error}<p class="error-text">{error}</p>{/if}
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
    padding: 10px 12px;
    border-radius: 8px;
    background: var(--bg-soft);
    margin: 0 0 12px;
  }
  .wrap {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
    background: var(--bg-soft);
  }
  .login {
    width: 100%;
    max-width: 400px;
    padding: 32px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .brand {
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .brand p {
    margin: 0;
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
  }
</style>
