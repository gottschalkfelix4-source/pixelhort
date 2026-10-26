import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { installLinkInterceptor } from './lib/router.svelte';
import { applyTheme, ui } from './lib/state.svelte';

applyTheme();
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if (ui.theme === 'system') applyTheme();
});
installLinkInterceptor();

const app = mount(App, { target: document.getElementById('app')! });

export default app;
