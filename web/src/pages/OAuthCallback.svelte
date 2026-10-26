<script lang="ts">
  import { api } from '../lib/api';
  import { loggedIn } from '../lib/state.svelte';
  import { navigate } from '../lib/router.svelte';
  import { t } from '../lib/i18n';

  let error = $state('');
  let started = false;

  $effect(() => {
    if (started) return;
    started = true;
    (async () => {
      try {
        const res = await api.oauthCallback(location.href);
        loggedIn(res.accessToken, res.user);
        let target = '/photos';
        try {
          target = sessionStorage.getItem('ph_oauth_redirect') || '/photos';
          sessionStorage.removeItem('ph_oauth_redirect');
        } catch {
          /* ignore */
        }
        navigate(target.startsWith('/') && !target.startsWith('//') ? target : '/photos', { replace: true });
      } catch (e) {
        error = e instanceof Error ? e.message : t('oauthFailed');
      }
    })();
  });
</script>

<div class="wrap">
  {#if error}
    <div class="card">
      <h2>{t('oauthFailed')}</h2>
      <p class="error-text">{error}</p>
      <a class="btn" href="/login">{t('back')}</a>
    </div>
  {:else}
    <div class="spinner"></div>
    <p class="muted">{t('oauthWorking')}</p>
  {/if}
</div>

<style>
  .wrap {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 16px;
    text-align: center;
  }
</style>
