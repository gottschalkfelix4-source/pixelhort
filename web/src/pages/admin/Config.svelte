<script lang="ts">
  import { api, type SystemConfig } from '../../lib/api';
  import { toast, toastError } from '../../lib/state.svelte';
  import { t } from '../../lib/i18n';

  let cfg = $state<SystemConfig | null>(null);
  let busy = $state(false);

  $effect(() => {
    api
      .getConfig()
      .then((c) => (cfg = c))
      .catch(toastError);
  });

  async function save(e: Event) {
    e.preventDefault();
    if (!cfg) return;
    busy = true;
    try {
      cfg = await api.putConfig($state.snapshot(cfg) as SystemConfig);
      toast(t('saved'), 'success');
    } catch (err) {
      toastError(err);
    } finally {
      busy = false;
    }
  }
</script>

{#if !cfg}
  <div class="spinner"></div>
{:else}
  <form class="cfg" onsubmit={save}>
    <fieldset class="card">
      <legend>{t('cfgMl')}</legend>
      <label class="check"><input type="checkbox" bind:checked={cfg.ml.enabled} /> {t('cfgEnabled')}</label>
      <div class="grid-2">
        <label class="field">{t('cfgMlUrl')}<input type="url" bind:value={cfg.ml.url} /></label>
        <label class="field">{t('cfgClipModel')}<input type="text" bind:value={cfg.ml.clipModel} /></label>
        <label class="field">{t('cfgFaceMinScore')}<input type="number" step="0.01" min="0" max="1" bind:value={cfg.ml.faceMinScore} /></label>
        <label class="field"
          >{t('cfgFaceMaxDistance')}<input type="number" step="0.01" min="0" max="2" bind:value={cfg.ml.faceMaxDistance} /></label
        >
        <label class="field">{t('cfgFaceMinFaces')}<input type="number" step="1" min="1" bind:value={cfg.ml.faceMinFaces} /></label>
        <label class="field"
          >{t('cfgDuplicateMaxDistance')}<input type="number" step="0.001" min="0" max="1" bind:value={cfg.ml.duplicateMaxDistance} /></label
        >
      </div>
      <label class="check"><input type="checkbox" bind:checked={cfg.ml.ocrEnabled} /> {t('cfgOcr')}</label>
      <label class="check"><input type="checkbox" bind:checked={cfg.ml.objectsEnabled} /> {t('cfgObjects')}</label>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgBackup')}</legend>
      <label class="check"><input type="checkbox" bind:checked={cfg.backup.enabled} /> {t('cfgEnabled')}</label>
      <div class="grid-2">
        <label class="field">{t('cfgCron')}<input type="text" bind:value={cfg.backup.cron} placeholder="0 0 2 * * *" /></label>
        <label class="field">{t('cfgKeep')}<input type="number" min="1" bind:value={cfg.backup.keep} /></label>
      </div>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgVideo')}</legend>
      <div class="grid-2">
        <label class="field"
          >{t('cfgTargetResolution')}
          <select bind:value={cfg.video.targetResolution}>
            {#each [480, 720, 1080, 1440, 2160] as r (r)}<option value={r}>{r}p</option>{/each}
          </select>
        </label>
        <label class="field">{t('cfgCrf')}<input type="number" min="0" max="51" bind:value={cfg.video.crf} /></label>
        <label class="field"
          >{t('cfgPreset')}
          <select bind:value={cfg.video.preset}>
            {#each ['ultrafast', 'superfast', 'veryfast', 'faster', 'fast', 'medium', 'slow', 'slower', 'veryslow'] as p (p)}<option value={p}
                >{p}</option
              >{/each}
          </select>
        </label>
      </div>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgTrash')} / {t('cfgReverseGeocoding')} / {t('cfgPasswordLogin')}</legend>
      <label class="field narrow">{t('cfgTrashDays')}<input type="number" min="0" bind:value={cfg.trash.days} /></label>
      <label class="check"><input type="checkbox" bind:checked={cfg.reverseGeocoding.enabled} /> {t('cfgReverseGeocoding')}</label>
      <label class="check"><input type="checkbox" bind:checked={cfg.passwordLogin.enabled} /> {t('cfgPasswordLogin')}</label>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgOauth')}</legend>
      <label class="check"><input type="checkbox" bind:checked={cfg.oauth.enabled} /> {t('cfgEnabled')}</label>
      <div class="grid-2">
        <label class="field">{t('cfgIssuerUrl')}<input type="url" bind:value={cfg.oauth.issuerUrl} /></label>
        <label class="field">{t('cfgClientId')}<input type="text" bind:value={cfg.oauth.clientId} /></label>
        <label class="field"
          >{t('cfgClientSecret')}<input type="password" bind:value={cfg.oauth.clientSecret} autocomplete="off" /></label
        >
        <label class="field">{t('cfgScope')}<input type="text" bind:value={cfg.oauth.scope} /></label>
        <label class="field">{t('cfgButtonText')}<input type="text" bind:value={cfg.oauth.buttonText} /></label>
        <label class="field">{t('cfgMobileRedirect')}<input type="text" bind:value={cfg.oauth.mobileRedirectUri} /></label>
      </div>
      <label class="check"><input type="checkbox" bind:checked={cfg.oauth.autoRegister} /> {t('cfgAutoRegister')}</label>
      <p class="muted small">Redirect URI (Web): <code>{location.origin}/auth/oauth-callback</code></p>
    </fieldset>

    <div class="actions">
      <button class="btn primary" disabled={busy}>{t('save')}</button>
    </div>
  </form>
{/if}

<style>
  .cfg {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 900px;
  }
  fieldset {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
  }
  legend {
    font-weight: 500;
    padding: 0 6px;
  }
  .narrow {
    max-width: 260px;
  }
  .actions {
    position: sticky;
    bottom: 16px;
    display: flex;
    justify-content: flex-end;
  }
</style>
