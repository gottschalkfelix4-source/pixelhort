# Pixelhort Web

Single-page web app for Pixelhort, built with Vite, Svelte 5 (runes) and TypeScript.
The REST contract is in [`../docs/API.md`](../docs/API.md). All API types and calls live in `src/lib/api.ts`.

## Development

```sh
npm install
npm run dev          # http://localhost:5173, /api is proxied to http://localhost:2342
PH_API_URL=http://192.168.1.10:2342 npm run dev   # use a different server
npm run check        # svelte-check (types + Svelte diagnostics)
npm run build        # production build to web/dist (served by the Rust server at "/")
```

The server serves `web/dist` with an SPA fallback to `index.html`. The API is same-origin under `/api`.

## Structure

```
src/
  main.ts, App.svelte         entry, routes, global drag & drop upload, viewer overlay
  app.css                     design tokens (light/dark), base components
  lib/
    api.ts                    typed REST client (types mirror docs/API.md), media URLs, XHR upload
    i18n.ts                   all UI strings (de default, en fallback)
    router.svelte.ts          small History API router
    state.svelte.ts           auth, theme, toasts, dialogs, viewer state
    selection.svelte.ts       multi-select model
    upload.svelte.ts          upload queue (SHA-1 → x-checksum, progress, duplicates)
    cast.ts                   Google Cast sender (loaded lazily)
    map.ts                    lazy maplibre-gl with OSM raster tiles
    utils.ts                  formatting, thumbhash, justified layout, downloads
  components/                 Shell, Timeline, AssetGrid, AssetViewer, InfoPanel, Editor,
                              SphereViewer (360°), MiniMap, Memories, SelectionBar, Pickers,
                              ShareLinkDialog, SearchFilterDialog, UploadPanel, Modal, Dialogs, Icon
  pages/                      one component per route (pages/admin/* are the admin tabs)
```

## Notes

- Auth: login sets the `ph_session` cookie. The token is also kept in `localStorage` and sent as
  `Authorization: Bearer` when the client makes API calls itself.
- Language: German is the default; English can be picked in Settings. Changing it reloads the page.
- maplibre-gl and photo-sphere-viewer are loaded on demand. They are split into separate chunks.
- Google Cast only works in Chromium-based browsers. Elsewhere the cast button shows a hint.
