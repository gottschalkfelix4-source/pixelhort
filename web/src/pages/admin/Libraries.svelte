<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Modal from '../../components/Modal.svelte';
  import { api, type LibraryDto, type UserDto } from '../../lib/api';
  import { auth, confirmDialog, toast, toastError } from '../../lib/state.svelte';
  import { formatAbs } from '../../lib/utils';
  import { t } from '../../lib/i18n';

  let libs = $state<LibraryDto[]>([]);
  let users = $state<UserDto[]>([]);
  let editing = $state<LibraryDto | 'new' | null>(null);
  let form = $state({ name: '', ownerId: '', importPaths: '', exclusionPatterns: '', watch: true });

  async function load() {
    try {
      [libs, users] = await Promise.all([api.libraries(), api.adminUsers()]);
    } catch (e) {
      toastError(e);
    }
  }
  $effect(() => {
    load();
  });

  function open(l: LibraryDto | 'new') {
    editing = l;
    form =
      l === 'new'
        ? { name: '', ownerId: auth.user?.id ?? '', importPaths: '', exclusionPatterns: '**/@eaDir/**\n**/.*/**', watch: true }
        : {
            name: l.name,
            ownerId: l.ownerId,
            importPaths: l.importPaths.join('\n'),
            exclusionPatterns: l.exclusionPatterns.join('\n'),
            watch: l.watch,
          };
  }

  const lines = (s: string) =>
    s
      .split('\n')
      .map((x) => x.trim())
      .filter(Boolean);

  async function save(e: Event) {
    e.preventDefault();
    const body = {
      name: form.name,
      ownerId: form.ownerId,
      importPaths: lines(form.importPaths),
      exclusionPatterns: lines(form.exclusionPatterns),
      watch: form.watch,
    };
    try {
      if (editing === 'new') await api.createLibrary(body);
      else if (editing) await api.updateLibrary(editing.id, body);
      editing = null;
      load();
    } catch (err) {
      toastError(err);
    }
  }

  async function scan(l: LibraryDto) {
    try {
      await api.scanLibrary(l.id);
      toast(t('scanStarted'), 'success');
    } catch (e) {
      toastError(e);
    }
  }

  async function remove(l: LibraryDto) {
    if (!(await confirmDialog(t('delete'), { message: t('deleteLibraryConfirm', { name: l.name }), danger: true, confirmLabel: t('delete') })))
      return;
    try {
      await api.deleteLibrary(l.id);
      load();
    } catch (e) {
      toastError(e);
    }
  }

  const ownerName = (id: string) => users.find((u) => u.id === id)?.name ?? id;
</script>

<div class="row head">
  <span class="spacer"></span>
  <button class="btn primary" onclick={() => open('new')}><Icon name="add" size={20} /> {t('newLibrary')}</button>
</div>
{#if !libs.length}
  <div class="empty">{t('nothingHere')}</div>
{/if}
<div class="libs">
  {#each libs as l (l.id)}
    <div class="card lib">
      <div class="row">
        <Icon name="folder" />
        <h3 class="grow">{l.name}</h3>
        <button class="btn small" onclick={() => scan(l)}><Icon name="refresh" size={16} /> {t('scan')}</button>
        <button class="icon-btn" onclick={() => open(l)} aria-label={t('edit')}><Icon name="edit" size={20} /></button>
        <button class="icon-btn" onclick={() => remove(l)} aria-label={t('delete')}><Icon name="delete" size={20} /></button>
      </div>
      <div class="muted small">
        {t('owner')}: {ownerName(l.ownerId)} · {t('photosCount', { n: l.assetCount })} · {t('lastScan')}: {l.refreshedAt
          ? formatAbs(l.refreshedAt)
          : t('never')}
        {#if l.watch}· {t('watch')}{/if}
      </div>
      <code>{l.importPaths.join('\n')}</code>
    </div>
  {/each}
</div>

{#if editing}
  <Modal title={editing === 'new' ? t('newLibrary') : t('edit')} onclose={() => (editing = null)} width={560}>
    <form class="form" id="lib-form" onsubmit={save}>
      <label class="field">{t('name')}<input type="text" bind:value={form.name} required /></label>
      <label class="field"
        >{t('owner')}
        <select bind:value={form.ownerId}>
          {#each users as u (u.id)}<option value={u.id}>{u.name} ({u.email})</option>{/each}
        </select>
      </label>
      <label class="field">{t('importPaths')}<textarea rows="3" bind:value={form.importPaths} placeholder="/mnt/photos"></textarea></label>
      <label class="field">{t('exclusionPatterns')}<textarea rows="3" bind:value={form.exclusionPatterns}></textarea></label>
      <label class="check"><input type="checkbox" bind:checked={form.watch} /> {t('watch')}</label>
    </form>
    {#snippet footer()}
      <button class="btn text" onclick={() => (editing = null)}>{t('cancel')}</button>
      <button class="btn primary" type="submit" form="lib-form">{t('save')}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .head {
    margin-bottom: 12px;
  }
  .libs {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .lib {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 16px;
  }
  .grow {
    flex: 1;
  }
  code {
    white-space: pre-wrap;
    font-size: 0.8rem;
    background: var(--bg-soft);
    padding: 6px 10px;
    border-radius: 6px;
  }
</style>
