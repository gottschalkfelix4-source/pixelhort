// Upload queue with SHA-1 checksums (x-checksum early duplicate check) and progress.
import { api, uploadAsset, type ShareAccess, type Uuid } from './api';
import { bumpAssets, deviceId } from './state.svelte';
import { sha1Hex } from './utils';

export type UploadStatus = 'hashing' | 'pending' | 'uploading' | 'done' | 'duplicate' | 'error';

export interface UploadItem {
  id: number;
  file: File;
  name: string;
  size: number;
  status: UploadStatus;
  progress: number;
  error?: string;
  assetId?: Uuid;
  checksum?: string;
  share?: ShareAccess | null;
  batch: number;
}

interface BatchOpts {
  share?: ShareAccess | null;
  albumId?: Uuid;
  onDone?: (assetIds: Uuid[]) => void;
}

export const ACCEPT =
  'image/*,video/*,.heic,.heif,.dng,.cr2,.cr3,.nef,.arw,.raf,.orf,.rw2,.srw,.pef,.raw,.3fr,.iiq,.x3f,.mts,.m2ts,.mkv,.insp,.jxl';

class UploadState {
  items = $state<UploadItem[]>([]);
  panelOpen = $state(false);
  active = $derived(this.items.filter((i) => i.status === 'hashing' || i.status === 'pending' || i.status === 'uploading').length);
  stats = $derived({
    total: this.items.length,
    done: this.items.filter((i) => i.status === 'done').length,
    dup: this.items.filter((i) => i.status === 'duplicate').length,
    err: this.items.filter((i) => i.status === 'error').length,
  });
}
export const uploads = new UploadState();

let nextId = 1;
let nextBatch = 1;
const batches = new Map<number, BatchOpts & { remaining: number; ids: Uuid[] }>();
const CONCURRENCY = 3;
let running = 0;

function isMedia(f: File) {
  if (f.type.startsWith('image/') || f.type.startsWith('video/')) return true;
  return /\.(heic|heif|dng|cr2|cr3|nef|arw|raf|orf|rw2|srw|pef|raw|3fr|iiq|x3f|mts|m2ts|mkv|insp|jxl)$/i.test(f.name);
}

export function addFiles(files: FileList | File[], opts: BatchOpts = {}) {
  const list = Array.from(files).filter(isMedia);
  if (!list.length) return;
  const batch = nextBatch++;
  batches.set(batch, { ...opts, remaining: list.length, ids: [] });
  for (const file of list) {
    uploads.items.push({
      id: nextId++,
      file,
      name: file.name,
      size: file.size,
      status: 'pending',
      progress: 0,
      share: opts.share,
      batch,
    });
  }
  uploads.panelOpen = true;
  pump();
}

function find(id: number) {
  return uploads.items.find((i) => i.id === id);
}

function pump() {
  while (running < CONCURRENCY) {
    const item = uploads.items.find((i) => i.status === 'pending' && !inFlight.has(i.id));
    if (!item) break;
    inFlight.add(item.id);
    running++;
    process(item.id).finally(() => {
      running--;
      inFlight.delete(item.id);
      pump();
    });
  }
}
const inFlight = new Set<number>();

async function process(id: number) {
  let item = find(id);
  if (!item) return;
  try {
    item.status = 'hashing';
    const checksum = await sha1Hex(item.file).catch(() => undefined);
    item = find(id);
    if (!item) return;
    item.checksum = checksum;
    item.status = 'uploading';
    const file = item.file;
    const res = await uploadAsset({
      file,
      checksum,
      deviceAssetId: `${file.name}-${file.size}-${file.lastModified}`,
      deviceId: deviceId(),
      share: item.share,
      onProgress: (f) => {
        const it = find(id);
        if (it) it.progress = f;
      },
    });
    item = find(id);
    if (!item) return;
    item.progress = 1;
    item.assetId = res.id;
    item.status = res.status === 'duplicate' ? 'duplicate' : 'done';
  } catch (e) {
    const it = find(id);
    if (it) {
      it.status = 'error';
      it.error = e instanceof Error ? e.message : String(e);
    }
  } finally {
    const it = find(id);
    if (it) finishBatchItem(it);
  }
}

async function finishBatchItem(item: UploadItem) {
  const b = batches.get(item.batch);
  if (!b) return;
  if (item.assetId) b.ids.push(item.assetId);
  b.remaining--;
  if (b.remaining > 0) return;
  batches.delete(item.batch);
  if (b.albumId && b.ids.length && !b.share) {
    try {
      await api.addAlbumAssets(b.albumId, b.ids);
    } catch {
      /* ignore */
    }
  }
  b.onDone?.(b.ids);
  bumpAssets();
}

export function clearFinished() {
  uploads.items = uploads.items.filter((i) => i.status === 'hashing' || i.status === 'pending' || i.status === 'uploading');
  if (!uploads.items.length) uploads.panelOpen = false;
}

/** Opens a native file picker and queues the selected files. */
export function pickFiles(opts: BatchOpts = {}) {
  const input = document.createElement('input');
  input.type = 'file';
  input.multiple = true;
  input.accept = ACCEPT;
  input.onchange = () => {
    if (input.files) addFiles(input.files, opts);
  };
  input.click();
}
