<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import { t } from '../lib/i18n';

  let {
    title,
    onclose,
    children,
    footer,
    width = 480,
  }: {
    title: string;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
    width?: number;
  } = $props();

  function onkey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title} style="max-width: {width}px">
    <header>
      <h2>{title}</h2>
      <button class="icon-btn" onclick={onclose} aria-label={t('close')}><Icon name="close" /></button>
    </header>
    <div class="body">
      {@render children()}
    </div>
    {#if footer}
      <footer>{@render footer()}</footer>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
  }
  .modal {
    background: var(--bg-elev);
    color: var(--fg);
    border-radius: 16px;
    box-shadow: var(--shadow);
    width: 100%;
    max-height: calc(100vh - 32px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    padding: 12px 8px 4px 20px;
    gap: 8px;
  }
  header h2 {
    flex: 1;
    min-width: 0;
  }
  .body {
    padding: 8px 20px 16px;
    overflow-y: auto;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px 16px;
    flex-wrap: wrap;
  }
  @media (max-width: 600px) {
    .backdrop {
      padding: 0;
      align-items: flex-end;
    }
    .modal {
      border-radius: 16px 16px 0 0;
      max-height: 92vh;
    }
  }
</style>
