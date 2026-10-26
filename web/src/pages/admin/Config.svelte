<script lang="ts">
  import { api, type SystemConfig } from '../../lib/api';
  import { auth, toast, toastError } from '../../lib/state.svelte';
  import { t } from '../../lib/i18n';

  let cfg = $state<SystemConfig | null>(null);
  let busy = $state(false);
  let quotaGb = $state('');
  let accelTest = $state<{ ok: boolean; encoder?: string; error?: string; devices: string[] } | null>(null);
  let testing = $state(false);

  async function testAccel() {
    if (!cfg) return;
    testing = true;
    accelTest = null;
    try {
      accelTest = await api.testAccel(cfg.video.accel, cfg.video.accelDevice);
    } catch (err) {
      toastError(err);
    } finally {
      testing = false;
    }
  }

  function loaded(c: SystemConfig) {
    cfg = c;
    quotaGb = c.user.defaultQuotaBytes ? String(c.user.defaultQuotaBytes / 1024 ** 3) : '';
  }

  $effect(() => {
    api.getConfig().then(loaded).catch(toastError);
  });

  /** Cleared number inputs bind to null; drop them so the server keeps the old value. */
  function withoutNulls(v: unknown): unknown {
    if (Array.isArray(v) || v === null || typeof v !== 'object') return v;
    return Object.fromEntries(
      Object.entries(v)
        .filter(([, x]) => x !== null && x !== undefined && !(typeof x === 'number' && isNaN(x)))
        .map(([k, x]) => [k, withoutNulls(x)]),
    );
  }

  async function save(e: Event) {
    e.preventDefault();
    if (!cfg) return;
    busy = true;
    try {
      const patch = withoutNulls($state.snapshot(cfg)) as SystemConfig;
      const gb = parseFloat(String(quotaGb ?? '').replace(',', '.'));
      patch.user = { defaultQuotaBytes: gb > 0 ? Math.round(gb * 1024 ** 3) : null };
      loaded(await api.putConfig(patch));
      api
        .serverInfo()
        .then((i) => (auth.serverInfo = i))
        .catch(() => {});
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
      <legend>{t('cfgServer')}</legend>
      <label class="field"
        >{t('cfgExternalDomain')}<input type="url" bind:value={cfg.server.externalDomain} placeholder="https://photos.example.com" /></label
      >
      <p class="muted small">{t('cfgExternalDomainHint')}</p>
      <label class="field">{t('cfgLoginMessage')}<textarea rows="2" bind:value={cfg.server.loginPageMessage}></textarea></label>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgUsers')}</legend>
      <label class="field narrow">{t('cfgDefaultQuota')}<input type="number" min="0" step="any" bind:value={quotaGb} /></label>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgMl')}</legend>
      <label class="setting"><span class="text"><strong>{t('cfgEnabled')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.ml.enabled} /></label>
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
      <label class="setting"><span class="text"><strong>{t('cfgOcr')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.ml.ocrEnabled} /></label>
      <label class="setting"><span class="text"><strong>{t('cfgObjects')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.ml.objectsEnabled} /></label>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgBackup')}</legend>
      <label class="setting"><span class="text"><strong>{t('cfgEnabled')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.backup.enabled} /></label>
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
        <label class="field"
          >{t('cfgAccel')}
          <select bind:value={cfg.video.accel}>
            <option value="none">{t('cfgAccelNone')}</option>
            <option value="vaapi">VAAPI (Intel)</option>
            <option value="qsv">Quick Sync (Intel)</option>
          </select>
        </label>
        {#if cfg.video.accel !== 'none'}
          <label class="field">{t('cfgAccelDevice')}<input type="text" bind:value={cfg.video.accelDevice} placeholder="/dev/dri/renderD128" /></label>
        {/if}
      </div>
      <p class="muted small">{t('cfgAccelHint')}</p>
      {#if cfg.video.accel !== 'none'}
        <div class="accel-test">
          <button type="button" class="btn" disabled={testing} onclick={testAccel}>{testing ? t('cfgAccelTesting') : t('cfgAccelTest')}</button>
          {#if accelTest}
            <span class={accelTest.ok ? 'ok' : 'err'}
              >{accelTest.ok ? t('cfgAccelOk', { encoder: accelTest.encoder ?? '' }) : t('cfgAccelFailed')}</span
            >
          {/if}
        </div>
        {#if accelTest}
          <p class="muted small">
            {accelTest.devices.length ? t('cfgAccelDevices', { list: accelTest.devices.join(', ') }) : t('cfgAccelNoDevices')}
          </p>
          {#if accelTest.error}<pre class="accel-error">{accelTest.error}</pre>{/if}
        {/if}
      {/if}
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgJobs')}</legend>
      <div class="grid-2">
        <label class="field">{t('cfgJobsMetadata')}<input type="number" min="1" max="8" step="1" bind:value={cfg.jobs.metadata} /></label>
        <label class="field">{t('cfgJobsThumbnails')}<input type="number" min="1" max="8" step="1" bind:value={cfg.jobs.thumbnails} /></label>
        <label class="field">{t('cfgJobsVideo')}<input type="number" min="1" max="8" step="1" bind:value={cfg.jobs.video} /></label>
      </div>
      <p class="muted small">{t('cfgJobsHint')}</p>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgImage')}</legend>
      <div class="grid-2">
        <label class="field"
          >{t('cfgPreviewSize')}<input type="number" min="320" max="4096" step="1" bind:value={cfg.image.previewSize} /></label
        >
        <label class="field"
          >{t('cfgPreviewQuality')}<input type="number" min="30" max="100" step="1" bind:value={cfg.image.previewQuality} /></label
        >
        <label class="field"
          >{t('cfgThumbSize')}<input type="number" min="64" max="1024" step="1" bind:value={cfg.image.thumbnailSize} /></label
        >
        <label class="field"
          >{t('cfgThumbQuality')}<input type="number" min="30" max="100" step="1" bind:value={cfg.image.thumbnailQuality} /></label
        >
      </div>
      <p class="muted small">{t('cfgImageHint')}</p>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgLibrary')}</legend>
      <label class="setting"><span class="text"><strong>{t('cfgLibraryScan')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.library.scanEnabled} /></label>
      <label class="field narrow">{t('cfgCron')}<input type="text" bind:value={cfg.library.scanCron} placeholder="0 0 * * * *" /></label>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgMap')}</legend>
      <label class="setting"><span class="text"><strong>{t('cfgEnabled')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.map.enabled} /></label>
      <label class="field">{t('cfgTileUrl')}<input type="text" bind:value={cfg.map.tileUrl} /></label>
      <label class="field">{t('cfgAttribution')}<input type="text" bind:value={cfg.map.attribution} /></label>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgTrash')} / {t('cfgReverseGeocoding')} / {t('cfgPasswordLogin')}</legend>
      <label class="field narrow">{t('cfgTrashDays')}<input type="number" min="0" bind:value={cfg.trash.days} /></label>
      <label class="setting"><span class="text"><strong>{t('cfgReverseGeocoding')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.reverseGeocoding.enabled} /></label>
      <label class="setting"><span class="text"><strong>{t('cfgPasswordLogin')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.passwordLogin.enabled} /></label>
    </fieldset>

    <fieldset class="card">
      <legend>{t('cfgOauth')}</legend>
      <label class="setting"><span class="text"><strong>{t('cfgEnabled')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.oauth.enabled} /></label>
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
      <label class="setting"><span class="text"><strong>{t('cfgAutoRegister')}</strong></span><input type="checkbox" class="switch" bind:checked={cfg.oauth.autoRegister} /></label>
      <p class="muted small">{t('cfgRedirectWeb')}: <code>{location.origin}/auth/oauth-callback</code></p>
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
    gap: 14px;
    margin: 0;
    min-width: 0;
  }
  legend {
    float: left;
    width: 100%;
    font-weight: 600;
    font-size: 1.05rem;
    padding: 0;
    margin-bottom: 4px;
  }
  fieldset :global(.setting) {
    padding: 6px 0;
  }
  fieldset :global(.setting + .setting) {
    border-top: none;
  }
  .narrow {
    max-width: 260px;
  }
  .accel-test {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .accel-test .ok {
    color: var(--success);
  }
  .accel-test .err {
    color: var(--danger);
  }
  .accel-error {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--bg-soft);
    font-size: 0.8rem;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 220px;
    overflow: auto;
  }
  .actions {
    position: sticky;
    bottom: 16px;
    display: flex;
    justify-content: flex-end;
  }
</style>
