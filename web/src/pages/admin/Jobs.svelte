<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import { api, JOB_QUEUES, type JobCommand, type QueueStatus } from '../../lib/api';
  import { toast, toastError } from '../../lib/state.svelte';
  import { t } from '../../lib/i18n';

  let jobs = $state<Record<string, QueueStatus>>({});
  let auto = $state(true);

  async function load() {
    try {
      jobs = await api.jobs();
    } catch (e) {
      toastError(e);
      auto = false;
    }
  }

  $effect(() => {
    load();
    if (!auto) return;
    const id = setInterval(load, 3000);
    return () => clearInterval(id);
  });

  let errors = $state<Record<string, { assetId: string | null; fileName: string | null; error: string | null }[] | undefined>>({});

  async function toggleErrors(queue: string) {
    if (errors[queue]) {
      errors[queue] = undefined;
      return;
    }
    try {
      errors[queue] = await api.failedJobs(queue);
    } catch (e) {
      toastError(e);
    }
  }

  const queues = $derived([...new Set([...JOB_QUEUES, ...Object.keys(jobs)])]);

  async function cmd(queue: string, command: JobCommand, force = false) {
    try {
      await api.jobCommand(queue, command, force);
      if (command === 'start') toast(t('jobStarted'), 'success');
      load();
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="row head">
  <label class="check small"><input type="checkbox" class="switch" bind:checked={auto} /> {t('autoRefresh')}</label>
  <span class="spacer"></span>
  <button class="icon-btn" onclick={load} aria-label={t('refresh')}><Icon name="refresh" /></button>
</div>
<div class="jobs">
  {#each queues as q (q)}
    {@const s = jobs[q]}
    <div class="card job" class:paused={s?.paused}>
      <div class="row">
        <h3>{q}</h3>
        {#if s?.paused}<span class="badge">{t('jobPaused')}</span>{/if}
        {#if s?.active}<span class="badge live">{t('jobActive')}</span>{/if}
      </div>
      <div class="stats">
        <div><strong>{s?.active ?? 0}</strong><span>{t('jobActive')}</span></div>
        <div><strong>{s?.waiting ?? 0}</strong><span>{t('jobWaiting')}</span></div>
        <div class:err={(s?.failed ?? 0) > 0}><strong>{s?.failed ?? 0}</strong><span>{t('jobFailed')}</span></div>
        <div><strong>{s?.completed ?? 0}</strong><span>{t('jobCompleted')}</span></div>
      </div>
      <div class="row wrap">
        <button class="btn small primary" onclick={() => cmd(q, 'start')}><Icon name="play" size={16} /> {t('jobStart')}</button>
        <button class="btn small" onclick={() => cmd(q, 'start', true)}>{t('jobStartAll')}</button>
        {#if s?.paused}
          <button class="btn small" onclick={() => cmd(q, 'resume')}>{t('jobResume')}</button>
        {:else}
          <button class="btn small" onclick={() => cmd(q, 'pause')}><Icon name="pause" size={16} /> {t('jobPause')}</button>
        {/if}
        {#if (s?.failed ?? 0) > 0}
          <button class="btn small" onclick={() => cmd(q, 'retry-failed')}><Icon name="refresh" size={16} /> {t('jobRetryFailed')}</button>
          <button class="btn small danger" onclick={() => cmd(q, 'clear-failed')}>{t('jobClearFailed')}</button>
          <button class="btn small" onclick={() => toggleErrors(q)}>{errors[q] ? t('jobHideErrors') : t('jobShowErrors')}</button>
        {/if}
      </div>
      {#if errors[q]}
        <ul class="errors">
          {#each errors[q] ?? [] as e, i (i)}
            <li><strong>{e.fileName ?? e.assetId ?? '–'}</strong><code>{e.error}</code></li>
          {/each}
        </ul>
      {/if}
    </div>
  {/each}
</div>

<style>
  .head {
    margin-bottom: 12px;
  }
  .jobs {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 16px;
  }
  .job {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 20px;
  }
  .job h3 {
    text-transform: capitalize;
  }
  .job.paused {
    opacity: 0.75;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    text-align: center;
  }
  .stats div {
    display: flex;
    flex-direction: column;
    padding: 10px 4px;
    border-radius: 12px;
    background: var(--bg-soft);
  }
  .stats strong {
    font-size: 1.3rem;
    font-weight: 700;
  }
  .stats span {
    font-size: 0.72rem;
    color: var(--fg-muted);
  }
  .stats .err strong {
    color: var(--danger);
  }
  .errors {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 320px;
    overflow: auto;
    font-size: 0.8rem;
  }
  .errors li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--bg-soft);
  }
  .errors code {
    color: var(--fg-muted);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .badge.live {
    background: var(--primary-soft);
    color: var(--primary);
  }
</style>
