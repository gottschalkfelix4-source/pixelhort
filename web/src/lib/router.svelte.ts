// Tiny History-API router built on Svelte 5 runes.

class RouterState {
  path = $state(location.pathname);
  search = $state(location.search);
  query = $derived(new URLSearchParams(this.search));
}

export const router = new RouterState();

function sync() {
  router.path = location.pathname;
  router.search = location.search;
}

window.addEventListener('popstate', sync);

export function navigate(to: string, opts: { replace?: boolean; keepScroll?: boolean } = {}) {
  const url = new URL(to, location.origin);
  const target = url.pathname + url.search + url.hash;
  if (target === location.pathname + location.search + location.hash) return;
  if (opts.replace) history.replaceState(null, '', target);
  else history.pushState(null, '', target);
  sync();
  if (!opts.keepScroll) window.scrollTo(0, 0);
}

/** Update query parameters of the current URL (null/'' removes). */
export function setQuery(params: Record<string, string | null | undefined>, replace = true) {
  const p = new URLSearchParams(location.search);
  for (const [k, v] of Object.entries(params)) {
    if (v === null || v === undefined || v === '') p.delete(k);
    else p.set(k, v);
  }
  const s = p.toString();
  navigate(location.pathname + (s ? `?${s}` : ''), { replace, keepScroll: true });
}

/** Match `/albums/:id` style patterns. Returns params or null. `*` matches the rest. */
export function matchRoute(pattern: string, path: string): Record<string, string> | null {
  const pp = pattern.split('/').filter(Boolean);
  const sp = path.split('/').filter(Boolean);
  const params: Record<string, string> = {};
  for (let i = 0; i < pp.length; i++) {
    const seg = pp[i];
    if (seg === '*') {
      params['rest'] = sp.slice(i).map(decodeURIComponent).join('/');
      return params;
    }
    if (i >= sp.length) {
      if (seg.endsWith('?')) continue;
      return null;
    }
    if (seg.startsWith(':')) {
      params[seg.slice(1).replace(/\?$/, '')] = decodeURIComponent(sp[i]);
    } else if (seg !== sp[i]) {
      return null;
    }
  }
  if (sp.length > pp.length) return null;
  return params;
}

/** Intercept clicks on same-origin <a href="/..."> links. */
export function installLinkInterceptor() {
  document.addEventListener('click', (e) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    const a = (e.target as Element | null)?.closest?.('a');
    if (!a) return;
    const href = a.getAttribute('href');
    if (!href || !href.startsWith('/') || href.startsWith('/api/') || href.startsWith('//')) return;
    if (a.target && a.target !== '_self') return;
    if (a.hasAttribute('download') || a.dataset.external !== undefined) return;
    e.preventDefault();
    navigate(href);
  });
}
