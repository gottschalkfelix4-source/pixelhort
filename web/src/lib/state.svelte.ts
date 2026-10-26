// Global application state (Svelte 5 runes).
import {
  api,
  setToken,
  type AssetDto,
  type AuthStatus,
  type DeepPartial,
  type ServerInfo,
  type ShareAccess,
  type UserDto,
  type UserPreferences,
} from './api';

// ───────────── Auth ─────────────
class AuthState {
  user = $state<UserDto | null>(null);
  status = $state<AuthStatus | null>(null);
  serverInfo = $state<ServerInfo | null>(null);
  ready = $state(false);
}
export const auth = new AuthState();

// ───────────── User preferences (stored on the server) ─────────────
export const defaultPreferences: UserPreferences = {
  memories: { enabled: true },
  people: { enabled: true },
  tags: { enabled: true },
  ratings: { enabled: true },
  sharedLinks: { enabled: true },
  download: { includeLiveVideo: false },
};
class PrefsState {
  value = $state<UserPreferences>(structuredClone(defaultPreferences));
}
export const prefs = new PrefsState();

export function loadPreferences() {
  api
    .preferences()
    .then((p) => (prefs.value = p))
    .catch(() => {});
}

export async function updatePreferences(patch: DeepPartial<UserPreferences>) {
  prefs.value = await api.updatePreferences(patch);
}

export async function initAuth() {
  try {
    auth.status = await api.authStatus();
  } catch {
    auth.status = null;
  }
  api
    .serverInfo()
    .then((i) => {
      auth.serverInfo = i;
    })
    .catch(() => {});
  if (auth.status && !auth.status.setupRequired) {
    try {
      auth.user = await api.me(true);
      loadPreferences();
    } catch {
      auth.user = null;
    }
  }
  auth.ready = true;
}

export function loggedIn(token: string, user: UserDto) {
  setToken(token);
  auth.user = user;
  if (auth.status) auth.status.setupRequired = false;
  loadPreferences();
}

export async function logout() {
  try {
    await api.logout();
  } catch {
    /* ignore */
  }
  setToken(null);
  auth.user = null;
}

// ───────────── Theme ─────────────
export type ThemePref = 'system' | 'light' | 'dark';
function readPref<T extends string>(key: string, fallback: T): T {
  try {
    return (localStorage.getItem(key) as T) || fallback;
  } catch {
    return fallback;
  }
}
export function writePref(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* ignore */
  }
}

class UiState {
  theme = $state<ThemePref>(readPref<ThemePref>('ph_theme', 'system'));
  sidebarOpen = $state(false);
  withPartners = $state(readPref<string>('ph_with_partners', 'false') === 'true');
  /** Per-browser playback settings. */
  videoAutoplay = $state(readPref<string>('ph_video_autoplay', 'true') === 'true');
  videoLoop = $state(readPref<string>('ph_video_loop', 'false') === 'true');
  videoMuted = $state(readPref<string>('ph_video_muted', 'false') === 'true');
  /** Incremented whenever assets change (upload, delete, ...) so lists reload. */
  assetsVersion = $state(0);
}
export const ui = new UiState();

export function applyTheme() {
  const dark =
    ui.theme === 'dark' || (ui.theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}
export function setTheme(t: ThemePref) {
  ui.theme = t;
  writePref('ph_theme', t);
  applyTheme();
}
export function bumpAssets() {
  ui.assetsVersion++;
}

// ───────────── Toasts ─────────────
export interface Toast {
  id: number;
  message: string;
  kind: 'info' | 'success' | 'error';
}
class ToastState {
  items = $state<Toast[]>([]);
}
export const toasts = new ToastState();
let toastId = 0;
export function toast(message: string, kind: Toast['kind'] = 'info', ms = 3500) {
  const id = ++toastId;
  toasts.items.push({ id, message, kind });
  setTimeout(() => {
    toasts.items = toasts.items.filter((t) => t.id !== id);
  }, ms);
}
export function toastError(e: unknown) {
  toast(e instanceof Error ? e.message : String(e), 'error', 5000);
}

// ───────────── Dialogs (confirm / prompt) ─────────────
export interface DialogRequest {
  kind: 'confirm' | 'prompt';
  title: string;
  message?: string;
  confirmLabel?: string;
  danger?: boolean;
  value?: string;
  inputType?: string;
  placeholder?: string;
  resolve: (v: string | boolean | null) => void;
}
class DialogState {
  current = $state<DialogRequest | null>(null);
}
export const dialogs = new DialogState();

export function confirmDialog(
  title: string,
  opts: { message?: string; confirmLabel?: string; danger?: boolean } = {},
): Promise<boolean> {
  return new Promise((resolve) => {
    dialogs.current = { kind: 'confirm', title, ...opts, resolve: (v) => resolve(v === true) };
  });
}
export function promptDialog(
  title: string,
  opts: { message?: string; value?: string; confirmLabel?: string; inputType?: string; placeholder?: string } = {},
): Promise<string | null> {
  return new Promise((resolve) => {
    dialogs.current = {
      kind: 'prompt',
      title,
      ...opts,
      resolve: (v) => resolve(typeof v === 'string' ? v : null),
    };
  });
}

// ───────────── Asset viewer ─────────────
export interface ViewerContext {
  assets: AssetDto[];
  index: number;
  share?: ShareAccess | null;
  allowDownload?: boolean;
  showMetadata?: boolean;
  readOnly?: boolean;
  /** Called after the viewer changed/removed an asset. */
  onChange?: () => void;
}
class ViewerState {
  ctx = $state<ViewerContext | null>(null);
}
export const viewer = new ViewerState();

export function openViewer(ctx: ViewerContext) {
  viewer.ctx = ctx;
}
export function closeViewer() {
  viewer.ctx = null;
}

// ───────────── Device id (upload) ─────────────
export function deviceId(): string {
  let id = readPref('ph_device_id', '');
  if (!id) {
    id = 'web-' + (crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2));
    writePref('ph_device_id', id);
  }
  return id;
}
