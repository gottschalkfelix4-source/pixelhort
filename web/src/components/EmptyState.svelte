<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';

  let {
    icon = 'photo',
    title,
    text = '',
    children,
  }: {
    icon?: IconName;
    title: string;
    text?: string;
    /** optional call to action below the text */
    children?: Snippet;
  } = $props();
</script>

<div class="empty-state">
  <div class="art" aria-hidden="true">
    <span class="tile a"></span>
    <span class="tile b"></span>
    <span class="badge-icon"><Icon name={icon} size={30} /></span>
  </div>
  <h2>{title}</h2>
  {#if text}<p>{text}</p>{/if}
  {#if children}<div class="cta">{@render children()}</div>{/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 56px 24px 64px;
    margin: 8px auto;
    max-width: 640px;
    border: 1px dashed var(--border-strong);
    border-radius: 28px;
  }
  .art {
    position: relative;
    width: 120px;
    height: 92px;
    margin-bottom: 22px;
  }
  .tile {
    position: absolute;
    width: 64px;
    height: 64px;
    border-radius: 16px;
    background: var(--bg-soft);
    border: 1px solid var(--border);
  }
  .tile.a {
    left: 4px;
    top: 14px;
    transform: rotate(-10deg);
  }
  .tile.b {
    right: 4px;
    top: 14px;
    transform: rotate(10deg);
  }
  .badge-icon {
    position: absolute;
    left: 50%;
    top: 4px;
    transform: translateX(-50%);
    width: 76px;
    height: 76px;
    border-radius: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--primary);
    background: var(--primary-soft);
    box-shadow:
      0 0 0 6px var(--bg),
      var(--shadow-sm);
  }
  h2 {
    font-size: 1.1rem;
    margin-bottom: 6px;
  }
  p {
    margin: 0;
    color: var(--fg-muted);
    max-width: 420px;
    font-size: 0.92rem;
  }
  .cta {
    margin-top: 22px;
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    justify-content: center;
  }
</style>
