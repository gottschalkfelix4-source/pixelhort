<script lang="ts">
  import Modal from './Modal.svelte';
  import { dialogs, toasts } from '../lib/state.svelte';
  import { t } from '../lib/i18n';

  let value = $state('');
  let input = $state<HTMLInputElement | null>(null);

  $effect(() => {
    const d = dialogs.current;
    if (d?.kind === 'prompt') {
      value = d.value ?? '';
      queueMicrotask(() => input?.select());
    }
  });

  function finish(result: string | boolean | null) {
    const d = dialogs.current;
    dialogs.current = null;
    d?.resolve(result);
  }
</script>

{#if dialogs.current}
  {@const d = dialogs.current}
  <Modal title={d.title} onclose={() => finish(null)} width={420}>
    {#if d.message}<p class="muted">{d.message}</p>{/if}
    {#if d.kind === 'prompt'}
      <form
        onsubmit={(e) => {
          e.preventDefault();
          finish(value);
        }}
      >
        <!-- svelte-ignore a11y_autofocus -->
        <input
          bind:this={input}
          type={d.inputType ?? 'text'}
          bind:value
          placeholder={d.placeholder}
          autofocus
        />
      </form>
    {/if}
    {#snippet footer()}
      <button class="btn text" onclick={() => finish(null)}>{t('cancel')}</button>
      <button
        class="btn primary"
        class:danger={d.danger}
        onclick={() => finish(d.kind === 'prompt' ? value : true)}>{d.confirmLabel ?? t('ok')}</button
      >
    {/snippet}
  </Modal>
{/if}

<div class="toasts" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    <div class="toast {toast.kind}">{toast.message}</div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 3000;
    pointer-events: none;
    width: max-content;
    max-width: calc(100vw - 32px);
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    background: #1c1c20;
    color: #fff;
    padding: 12px 20px 12px 16px;
    border-radius: 14px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: var(--shadow);
    font-size: 0.9rem;
    font-weight: 500;
    animation: pop 0.2s ease-out;
  }
  .toast::before {
    content: '';
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #9ec2ff;
    flex-shrink: 0;
  }
  .toast.error::before {
    background: #f87171;
  }
  .toast.success::before {
    background: #4ade80;
  }
  @keyframes pop {
    from {
      transform: translateY(12px);
      opacity: 0;
    }
  }
</style>
