<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Modal from '../../components/Modal.svelte';
  import { api, type UserDto } from '../../lib/api';
  import { auth, confirmDialog, toast, toastError } from '../../lib/state.svelte';
  import { formatAbs, formatBytes } from '../../lib/utils';
  import { t } from '../../lib/i18n';

  let users = $state<UserDto[]>([]);
  let editing = $state<UserDto | 'new' | null>(null);
  let form = $state({ name: '', email: '', password: '', isAdmin: false, quotaGb: '' });

  async function load() {
    try {
      users = await api.adminUsers();
    } catch (e) {
      toastError(e);
    }
  }
  $effect(() => {
    load();
  });

  function open(u: UserDto | 'new') {
    editing = u;
    form =
      u === 'new'
        ? { name: '', email: '', password: '', isAdmin: false, quotaGb: '' }
        : { name: u.name, email: u.email, password: '', isAdmin: u.isAdmin, quotaGb: u.quotaBytes ? String(u.quotaBytes / 1024 ** 3) : '' };
  }

  async function save(e: Event) {
    e.preventDefault();
    const quotaBytes = form.quotaGb.trim() ? Math.round(parseFloat(form.quotaGb) * 1024 ** 3) : null;
    try {
      if (editing === 'new') {
        await api.adminCreateUser({ name: form.name, email: form.email, password: form.password, isAdmin: form.isAdmin, quotaBytes });
      } else if (editing) {
        await api.adminUpdateUser(editing.id, {
          name: form.name,
          email: form.email,
          isAdmin: form.isAdmin,
          quotaBytes,
          ...(form.password ? { password: form.password } : {}),
        });
      }
      editing = null;
      toast(t('saved'), 'success');
      load();
    } catch (err) {
      toastError(err);
    }
  }

  async function remove(u: UserDto) {
    if (!(await confirmDialog(t('delete'), { message: t('deleteUserConfirm', { name: u.name }), danger: true, confirmLabel: t('delete') })))
      return;
    try {
      await api.adminDeleteUser(u.id);
      load();
    } catch (e) {
      toastError(e);
    }
  }
</script>

<div class="row head">
  <span class="spacer"></span>
  <button class="btn primary" onclick={() => open('new')}><Icon name="add" size={20} /> {t('newUser')}</button>
</div>
<div class="table-wrap">
  <table class="table">
    <thead>
      <tr><th>{t('name')}</th><th>{t('email')}</th><th>{t('isAdmin')}</th><th>{t('usage')}</th><th>{t('createdAt')}</th><th></th></tr>
    </thead>
    <tbody>
      {#each users as u (u.id)}
        <tr>
          <td>{u.name}</td>
          <td>{u.email}</td>
          <td>{u.isAdmin ? t('yes') : t('no')}</td>
          <td>{formatBytes(u.usageBytes)}{u.quotaBytes ? ` / ${formatBytes(u.quotaBytes)}` : ''}</td>
          <td class="small muted">{formatAbs(u.createdAt, { dateStyle: 'medium' })}</td>
          <td class="acts">
            <button class="icon-btn" onclick={() => open(u)} aria-label={t('edit')}><Icon name="edit" size={20} /></button>
            {#if u.id !== auth.user?.id}
              <button class="icon-btn" onclick={() => remove(u)} aria-label={t('delete')}><Icon name="delete" size={20} /></button>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

{#if editing}
  <Modal title={editing === 'new' ? t('newUser') : t('edit')} onclose={() => (editing = null)}>
    <form class="form" id="user-form" onsubmit={save}>
      <label class="field">{t('name')}<input type="text" bind:value={form.name} required /></label>
      <label class="field">{t('email')}<input type="email" bind:value={form.email} required /></label>
      <label class="field"
        >{t('password')}{editing !== 'new' ? ` (${t('optional')})` : ''}<input
          type="password"
          bind:value={form.password}
          required={editing === 'new'}
          autocomplete="new-password"
        /></label
      >
      <label class="field">{t('quota')}<input type="number" min="0" step="0.1" bind:value={form.quotaGb} /></label>
      <label class="check"><input type="checkbox" bind:checked={form.isAdmin} /> {t('isAdmin')}</label>
    </form>
    {#snippet footer()}
      <button class="btn text" onclick={() => (editing = null)}>{t('cancel')}</button>
      <button class="btn primary" type="submit" form="user-form">{t('save')}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .head {
    margin-bottom: 8px;
  }
  .acts {
    white-space: nowrap;
    text-align: right;
  }
</style>
