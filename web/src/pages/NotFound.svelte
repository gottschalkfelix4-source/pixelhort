<script lang="ts">
  import Logo from '../components/Logo.svelte';
</script>

<div class="page">
  <div class="empty">
    <h1>404</h1>
    <p><a class="home" href="/photos"><Logo size={28} wordmark /></a></p>
  </div>
</div>

<style>
  .home {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 1.2rem;
  }
</style>
