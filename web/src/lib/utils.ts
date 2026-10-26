import { thumbHashToDataURL } from 'thumbhash';
import { originalUrl, type AssetDto, type ShareAccess } from './api';
import { locale } from './i18n';

// ───────────── Formatting ─────────────

export function formatBytes(n: number | null | undefined): string {
  if (n === null || n === undefined || isNaN(n)) return '–';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

export function formatDuration(sec: number | null | undefined): string {
  if (!sec) return '';
  const s = Math.round(sec);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  const mm = h ? String(m).padStart(2, '0') : String(m);
  return `${h ? h + ':' : ''}${mm}:${String(r).padStart(2, '0')}`;
}

/** `localDateTime` is wall-clock time encoded as UTC, so format in UTC. */
export function formatLocal(iso: string | null | undefined, opts: Intl.DateTimeFormatOptions): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return new Intl.DateTimeFormat(locale(), { ...opts, timeZone: 'UTC' }).format(d);
}

/** Format a real (absolute) timestamp in the browser's time zone. */
export function formatAbs(iso: string | null | undefined, opts: Intl.DateTimeFormatOptions = { dateStyle: 'medium', timeStyle: 'short' }): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return new Intl.DateTimeFormat(locale(), opts).format(d);
}

export function dayLabel(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const sameYear = d.getUTCFullYear() === now.getFullYear();
  return formatLocal(iso, { weekday: 'short', day: 'numeric', month: 'short', year: sameYear ? undefined : 'numeric' });
}

export function monthLabel(bucket: string): string {
  return formatLocal(bucket.length === 10 ? `${bucket}T00:00:00Z` : bucket, { month: 'long', year: 'numeric' });
}

export function dayKey(iso: string): string {
  return iso.slice(0, 10);
}

export function toDateInput(iso: string | null | undefined): string {
  return iso ? iso.slice(0, 10) : '';
}
/** datetime-local value (browser local time) from ISO. */
export function toDateTimeLocal(iso: string | null | undefined): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (isNaN(d.getTime())) return '';
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
export function fromDateTimeLocal(v: string): string | null {
  if (!v) return null;
  const d = new Date(v);
  return isNaN(d.getTime()) ? null : d.toISOString();
}

// ───────────── Thumbhash ─────────────

const hashCache = new Map<string, string>();
export function thumbhashUrl(hash: string | null | undefined): string | null {
  if (!hash) return null;
  const cached = hashCache.get(hash);
  if (cached) return cached;
  try {
    const bin = atob(hash.replace(/-/g, '+').replace(/_/g, '/'));
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    const url = thumbHashToDataURL(bytes);
    hashCache.set(hash, url);
    return url;
  } catch {
    return null;
  }
}

// ───────────── Justified layout ─────────────

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

export function aspectOf(a: AssetDto): number {
  if (a.width && a.height && a.width > 0 && a.height > 0) {
    const r = a.width / a.height;
    return Math.min(Math.max(r, 0.4), 3.5);
  }
  return 1;
}

export function justify(aspects: number[], width: number, targetHeight: number, gap = 4): { boxes: Box[]; height: number } {
  const boxes: Box[] = [];
  if (width <= 0) return { boxes: aspects.map(() => ({ left: 0, top: 0, width: 0, height: 0 })), height: 0 };
  let top = 0;
  let row: number[] = [];
  let rowStart = 0;
  const flush = (last: boolean) => {
    const sum = row.reduce((s, r) => s + r, 0);
    const avail = width - gap * (row.length - 1);
    let h = avail / sum;
    if (last && h > targetHeight) h = targetHeight;
    h = Math.min(h, targetHeight * 2);
    let left = 0;
    row.forEach((r, i) => {
      const w = i === row.length - 1 && !last ? width - left : r * h;
      boxes[rowStart + i] = { left, top, width: w, height: h };
      left += w + gap;
    });
    top += h + gap;
    rowStart += row.length;
    row = [];
  };
  for (let i = 0; i < aspects.length; i++) {
    row.push(aspects[i]);
    const sum = row.reduce((s, r) => s + r, 0);
    if (sum * targetHeight + gap * (row.length - 1) >= width) flush(false);
  }
  if (row.length) flush(true);
  return { boxes, height: Math.max(0, top - gap) };
}

// ───────────── Misc ─────────────

export async function sha1Hex(file: Blob): Promise<string | undefined> {
  if (!globalThis.crypto?.subtle) return undefined;
  if (file.size > 1024 * 1024 * 1024) return undefined; // avoid loading >1 GiB into memory
  const buf = await file.arrayBuffer();
  const digest = await crypto.subtle.digest('SHA-1', buf);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

export function triggerDownload(url: string, filename?: string) {
  const a = document.createElement('a');
  a.href = url;
  if (filename) a.download = filename;
  a.rel = 'noopener';
  document.body.appendChild(a);
  a.click();
  a.remove();
}

export async function downloadAssets(assets: AssetDto[], share?: ShareAccess | null) {
  for (const a of assets) {
    triggerDownload(originalUrl(a.id, share, a.hasEdits), a.originalFileName);
    // Browsers throttle multiple downloads; space them out a bit.
    await new Promise((r) => setTimeout(r, 400));
  }
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand('copy');
    ta.remove();
    return ok;
  }
}

export function debounce<A extends unknown[]>(fn: (...a: A) => void, ms: number) {
  let t: ReturnType<typeof setTimeout> | undefined;
  return (...a: A) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...a), ms);
  };
}

/** Browser-displayable original formats (for editor/zoom). */
export function isWebImage(mime: string): boolean {
  return /^image\/(jpeg|png|webp|gif|avif|bmp)$/i.test(mime);
}

export function absoluteUrl(path: string): string {
  return new URL(path, location.origin).toString();
}
