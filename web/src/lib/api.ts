// Typed client for the Pixelhort REST API (docs/API.md is the binding contract).
// All requests are same-origin under /api. Auth: `ph_session` cookie (set by
// login) plus an `Authorization: Bearer` header as fallback.

// ───────────────────────────── Types ─────────────────────────────

export type Uuid = string;
export type IsoDate = string;

export interface ApiErrorBody {
  statusCode: number;
  error: string;
  message: string;
}

export interface AuthStatus {
  setupRequired: boolean;
  oauthEnabled: boolean;
  oauthButtonText: string;
  passwordLoginEnabled: boolean;
  loginPageMessage?: string;
}

export interface UserDto {
  id: Uuid;
  email: string;
  name: string;
  isAdmin: boolean;
  quotaBytes: number | null;
  usageBytes: number;
  hasPin: boolean;
  oauthId: string | null;
  createdAt: IsoDate;
  updatedAt: IsoDate;
}

export interface UserSummary {
  id: Uuid;
  name: string;
  email: string;
}

export interface LoginResponse {
  accessToken: string;
  user: UserDto;
}

export interface SessionInfo {
  id: Uuid | null;
  unlockedUntil: IsoDate | null;
  hasPin: boolean;
}

export interface AuthSessionDto {
  id: Uuid;
  deviceType: string | null;
  deviceName: string | null;
  createdAt: IsoDate;
  lastUsedAt: IsoDate;
  current: boolean;
}

export interface UserPreferences {
  memories: { enabled: boolean };
  people: { enabled: boolean };
  tags: { enabled: boolean };
  ratings: { enabled: boolean };
  sharedLinks: { enabled: boolean };
  download: { includeLiveVideo: boolean };
}

export interface ApiKeyDto {
  id: Uuid;
  name: string;
  createdAt: IsoDate;
}
export interface ApiKeyCreated extends ApiKeyDto {
  secret: string;
}

export interface MlConfig {
  enabled: boolean;
  url: string;
  clipModel: string;
  faceMinScore: number;
  faceMaxDistance: number;
  faceMinFaces: number;
  duplicateMaxDistance: number;
  ocrEnabled: boolean;
  objectsEnabled: boolean;
}
export interface SystemConfig {
  ml: MlConfig;
  backup: { enabled: boolean; cron: string; keep: number };
  video: { targetResolution: number; crf: number; preset: string; accel: 'none' | 'vaapi' | 'qsv'; accelDevice: string };
  jobs: { metadata: number; thumbnails: number; video: number };
  trash: { days: number };
  reverseGeocoding: { enabled: boolean };
  passwordLogin: { enabled: boolean };
  oauth: {
    enabled: boolean;
    issuerUrl: string;
    clientId: string;
    clientSecret: string;
    scope: string;
    buttonText: string;
    autoRegister: boolean;
    mobileRedirectUri: string;
  };
  server: { externalDomain: string; loginPageMessage: string };
  user: { defaultQuotaBytes: number | null };
  library: { scanEnabled: boolean; scanCron: string };
  image: { previewSize: number; previewQuality: number; thumbnailSize: number; thumbnailQuality: number };
  map: { enabled: boolean; tileUrl: string; attribution: string };
}
export type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K] };

export interface BackupDto {
  name: string;
  size: number;
  createdAt: IsoDate;
}

export interface QueueStatus {
  active: number;
  waiting: number;
  failed: number;
  completed: number;
  paused: boolean;
}
export type JobQueue =
  | 'metadata'
  | 'thumbnails'
  | 'video'
  | 'smart-search'
  | 'faces'
  | 'facial-recognition'
  | 'ocr'
  | 'objects'
  | 'duplicates'
  | 'library'
  | 'backup'
  | 'cleanup';
export const JOB_QUEUES: JobQueue[] = [
  'metadata',
  'thumbnails',
  'video',
  'smart-search',
  'faces',
  'facial-recognition',
  'ocr',
  'objects',
  'duplicates',
  'library',
  'backup',
  'cleanup',
];
export type JobCommand = 'start' | 'pause' | 'resume' | 'clear-failed' | 'retry-failed';

export interface ServerInfo {
  version: string;
  features: {
    smartSearch: boolean;
    faces: boolean;
    ocr: boolean;
    objects: boolean;
    oauth: boolean;
    reverseGeocoding: boolean;
    cast: boolean;
    map?: boolean;
  };
  publicUrl: string | null;
  mapTileUrl?: string;
  mapAttribution?: string;
  trashDays?: number;
}

export interface ServerStatistics {
  photos: number;
  videos: number;
  usageBytes: number;
  users: { userId: Uuid; name: string; photos: number; videos: number; usageBytes: number }[];
}

export type AssetType = 'IMAGE' | 'VIDEO';
export type Visibility = 'timeline' | 'archive' | 'hidden' | 'locked';

export interface ExifDto {
  make: string | null;
  model: string | null;
  lensModel: string | null;
  fNumber: number | null;
  exposureTime: string | null;
  iso: number | null;
  focalLength: number | null;
  latitude: number | null;
  longitude: number | null;
  altitude: number | null;
  city: string | null;
  state: string | null;
  country: string | null;
  orientation: string | null;
  dateTimeOriginal: IsoDate | null;
  timeZone: string | null;
  projectionType: string | null;
  description: string | null;
}

export interface AssetTagRef {
  id: Uuid;
  value: string;
  color: string | null;
}
export interface AssetPersonRef {
  id: Uuid;
  name: string;
}

export interface AssetDto {
  id: Uuid;
  ownerId: Uuid;
  libraryId: Uuid | null;
  deviceAssetId: string;
  deviceId: string;
  type: AssetType;
  originalFileName: string;
  originalMimeType: string;
  checksum: string;
  fileSize: number;
  fileCreatedAt: IsoDate;
  fileModifiedAt: IsoDate;
  localDateTime: IsoDate;
  createdAt: IsoDate;
  updatedAt: IsoDate;
  width: number | null;
  height: number | null;
  duration: number | null;
  isFavorite: boolean;
  visibility: Visibility;
  isTrashed: boolean;
  isOffline: boolean;
  livePhotoVideoId: Uuid | null;
  isMotionPhoto: boolean;
  projectionType: 'EQUIRECTANGULAR' | null;
  isPanorama: boolean;
  isRaw: boolean;
  thumbhash: string | null;
  rating: number | null;
  description: string;
  hasEdits: boolean;
  duplicateId: Uuid | null;
  exif: ExifDto | null;
  tags: AssetTagRef[];
  people: AssetPersonRef[];
}

export interface UploadResult {
  id: Uuid;
  status: 'created' | 'duplicate';
}

export interface BulkUploadCheckResult {
  results: { id: string; action: 'accept' | 'reject'; reason?: string; assetId?: Uuid }[];
}

export interface AssetUpdate {
  isFavorite?: boolean;
  visibility?: Visibility;
  description?: string;
  rating?: number | null;
  dateTimeOriginal?: IsoDate;
  latitude?: number;
  longitude?: number;
}
export interface AssetBulkUpdate {
  ids: Uuid[];
  isFavorite?: boolean;
  visibility?: Visibility;
  rating?: number | null;
}

export type EditFilter = 'bw' | 'sepia' | 'warm' | 'cool' | 'vintage';
export interface EditsDto {
  crop: { x: number; y: number; width: number; height: number } | null;
  rotate: 0 | 90 | 180 | 270;
  flipHorizontal: boolean;
  flipVertical: boolean;
  brightness: number;
  contrast: number;
  saturation: number;
  filter: EditFilter | null;
}

export interface CastUrl {
  url: string;
  mimeType: string;
  imageUrl: string;
}

/** DLNA/UPnP media renderer (smart TV, media player) found on the server's network. */
export interface CastDevice {
  id: string;
  name: string;
  manufacturer: string | null;
  model: string | null;
  location: string;
}

export interface AssetStatistics {
  images: number;
  videos: number;
  total: number;
}

export interface TimelineFilters {
  visibility?: Visibility;
  isFavorite?: boolean;
  albumId?: Uuid;
  personId?: Uuid;
  tagId?: Uuid;
  withPartners?: boolean;
  isTrashed?: boolean;
  order?: 'desc' | 'asc';
}
export interface TimeBucket {
  timeBucket: string;
  count: number;
}

export interface SyncDelta {
  upserted: AssetDto[];
  deleted: Uuid[];
  hasMore: boolean;
  cursor: string;
}

export type AlbumRole = 'editor' | 'viewer';
export interface AlbumUser {
  user: UserSummary;
  role: AlbumRole;
}
export interface AlbumDto {
  id: Uuid;
  ownerId: Uuid;
  owner: UserSummary;
  albumName: string;
  description: string;
  albumThumbnailAssetId: Uuid | null;
  assetCount: number;
  startDate: IsoDate | null;
  endDate: IsoDate | null;
  createdAt: IsoDate;
  updatedAt: IsoDate;
  shared: boolean;
  order: 'asc' | 'desc';
  albumUsers: AlbumUser[];
  assets?: AssetDto[];
}
export interface BulkIdResult {
  id: Uuid;
  success: boolean;
  error?: 'duplicate' | 'no_permission';
}

export interface TagDto {
  id: Uuid;
  value: string;
  name: string;
  color: string | null;
  parentId: Uuid | null;
  assetCount: number;
}

export interface MapMarker {
  id: Uuid;
  lat: number;
  lon: number;
  city: string | null;
  state: string | null;
  country: string | null;
}

export interface Memory {
  id: string;
  type: string;
  yearsAgo: number;
  year: number;
  title: string;
  assets: AssetDto[];
}

export interface PersonDto {
  id: Uuid;
  name: string;
  birthDate: string | null;
  isHidden: boolean;
  faceCount: number;
  thumbnailUrl: string;
  updatedAt: IsoDate;
}
export interface PeopleResponse {
  people: PersonDto[];
  total: number;
  hidden: number;
}
export interface FaceDto {
  id: Uuid;
  personId: Uuid | null;
  person: PersonDto | null;
  boundingBoxX1: number;
  boundingBoxY1: number;
  boundingBoxX2: number;
  boundingBoxY2: number;
  imageWidth: number;
  imageHeight: number;
}

export interface SearchResponse {
  assets: AssetDto[];
  nextPage: number | null;
  total: number;
}
export interface MetadataSearch {
  originalFileName?: string;
  text?: string;
  make?: string;
  model?: string;
  lensModel?: string;
  city?: string;
  state?: string;
  country?: string;
  takenAfter?: IsoDate;
  takenBefore?: IsoDate;
  type?: AssetType;
  isFavorite?: boolean;
  personIds?: Uuid[];
  tagIds?: Uuid[];
  objects?: string[];
  isRaw?: boolean;
  isPanorama?: boolean;
  is360?: boolean;
  isMotion?: boolean;
  visibility?: Visibility;
  page?: number;
  size?: number;
}
export interface SmartSearch extends MetadataSearch {
  query: string;
}
export type SuggestionType = 'city' | 'country' | 'make' | 'model' | 'object';
export interface ExploreResponse {
  places: { value: string; assetId: Uuid }[];
  objects: { value: string; assetId: Uuid }[];
  people: PersonDto[];
}

export interface DuplicateGroup {
  duplicateId: Uuid;
  assets: AssetDto[];
}

export interface LibraryDto {
  id: Uuid;
  ownerId: Uuid;
  name: string;
  importPaths: string[];
  exclusionPatterns: string[];
  watch: boolean;
  assetCount: number;
  refreshedAt: IsoDate | null;
  createdAt: IsoDate;
}
export interface LibraryCreate {
  ownerId: Uuid;
  name: string;
  importPaths: string[];
  exclusionPatterns?: string[];
  watch?: boolean;
}

export interface PartnerDto {
  user: UserSummary;
  inTimeline: boolean;
}

export type SharedLinkType = 'ALBUM' | 'INDIVIDUAL';
export interface SharedLinkDto {
  id: Uuid;
  key: string;
  type: SharedLinkType;
  albumId: Uuid | null;
  album: AlbumDto | null;
  assetIds: Uuid[];
  description: string | null;
  hasPassword: boolean;
  expiresAt: IsoDate | null;
  allowDownload: boolean;
  allowUpload: boolean;
  showMetadata: boolean;
  createdAt: IsoDate;
  url: string;
}
export interface SharedLinkCreate {
  type: SharedLinkType;
  albumId?: Uuid;
  assetIds?: Uuid[];
  description?: string;
  password?: string;
  expiresAt?: IsoDate | null;
  allowDownload?: boolean;
  allowUpload?: boolean;
  showMetadata?: boolean;
  /** Update only: removes the expiry date. */
  clearExpiry?: boolean;
}
export interface SharedLinkMe {
  link: SharedLinkDto;
  assets: AssetDto[];
}

export interface ActivityDto {
  id: Uuid;
  user: UserSummary;
  type: 'like' | 'comment';
  comment: string | null;
  assetId: Uuid | null;
  createdAt: IsoDate;
}

/** Credentials for public share access (`?key=&shareToken=`). */
export interface ShareAccess {
  key: string;
  shareToken?: string;
}

// ───────────────────────────── Transport ─────────────────────────────

export class ApiError extends Error {
  status: number;
  body: ApiErrorBody | null;
  constructor(status: number, body: ApiErrorBody | null, fallback: string) {
    super(body?.message || fallback);
    this.status = status;
    this.body = body;
  }
}

const TOKEN_KEY = 'ph_token';
let token: string | null = null;
try {
  token = localStorage.getItem(TOKEN_KEY);
} catch {
  token = null;
}

export function setToken(t: string | null) {
  token = t;
  try {
    if (t) localStorage.setItem(TOKEN_KEY, t);
    else localStorage.removeItem(TOKEN_KEY);
  } catch {
    /* ignore */
  }
}
export function getToken() {
  return token;
}

/** Listeners notified on 401 responses (used to redirect to login). */
const unauthorizedListeners = new Set<() => void>();
export function onUnauthorized(fn: () => void) {
  unauthorizedListeners.add(fn);
  return () => unauthorizedListeners.delete(fn);
}

type Query = Record<string, string | number | boolean | null | undefined>;

export function qs(query?: Query): string {
  if (!query) return '';
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(query)) {
    if (v === undefined || v === null || v === '') continue;
    p.set(k, String(v));
  }
  const s = p.toString();
  return s ? `?${s}` : '';
}

function shareQuery(share?: ShareAccess | null): Query {
  return share ? { key: share.key, shareToken: share.shareToken } : {};
}

interface RequestOpts {
  query?: Query;
  body?: unknown;
  /** Do not trigger the global 401 handler (e.g. auth/status, share pages). */
  silent?: boolean;
  headers?: Record<string, string>;
}

async function request<T>(method: string, path: string, opts: RequestOpts = {}): Promise<T> {
  const headers: Record<string, string> = { Accept: 'application/json', ...opts.headers };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  let body: BodyInit | undefined;
  if (opts.body instanceof FormData) {
    body = opts.body;
  } else if (opts.body !== undefined) {
    headers['Content-Type'] = 'application/json';
    body = JSON.stringify(opts.body);
  }
  const res = await fetch(`/api${path}${qs(opts.query)}`, {
    method,
    headers,
    body,
    credentials: 'same-origin',
  });
  if (!res.ok) {
    let errBody: ApiErrorBody | null = null;
    try {
      errBody = (await res.json()) as ApiErrorBody;
    } catch {
      errBody = null;
    }
    if (res.status === 401 && !opts.silent) unauthorizedListeners.forEach((fn) => fn());
    throw new ApiError(res.status, errBody, `${res.status} ${res.statusText}`);
  }
  if (res.status === 204 || res.status === 202) return undefined as T;
  const text = await res.text();
  if (!text) return undefined as T;
  return JSON.parse(text) as T;
}

const get = <T>(path: string, query?: Query, opts: Omit<RequestOpts, 'query'> = {}) =>
  request<T>('GET', path, { ...opts, query });
const post = <T>(path: string, body?: unknown, opts: RequestOpts = {}) =>
  request<T>('POST', path, { ...opts, body });
const put = <T>(path: string, body?: unknown, opts: RequestOpts = {}) =>
  request<T>('PUT', path, { ...opts, body });
const patch = <T>(path: string, body?: unknown, opts: RequestOpts = {}) =>
  request<T>('PATCH', path, { ...opts, body });
const del = <T>(path: string, body?: unknown, opts: RequestOpts = {}) =>
  request<T>('DELETE', path, { ...opts, body });

// ───────────────────────────── Media URLs ─────────────────────────────

export type ThumbSize = 'thumbnail' | 'preview';

export function thumbnailUrl(id: Uuid, size: ThumbSize = 'thumbnail', share?: ShareAccess | null, cacheKey?: string) {
  return `/api/assets/${id}/thumbnail${qs({ size, ...shareQuery(share), c: cacheKey })}`;
}
export function originalUrl(id: Uuid, share?: ShareAccess | null, edited = false) {
  return `/api/assets/${id}/original${qs({ edited: edited || undefined, ...shareQuery(share) })}`;
}
export function videoUrl(id: Uuid, share?: ShareAccess | null) {
  return `/api/assets/${id}/video${qs(shareQuery(share))}`;
}
export function personThumbnailUrl(id: Uuid) {
  return `/api/people/${id}/thumbnail`;
}
export function backupDownloadUrl(name: string) {
  return `/api/admin/backups/${encodeURIComponent(name)}`;
}

// ───────────────────────────── Endpoints ─────────────────────────────

export const api = {
  // Auth
  authStatus: () => get<AuthStatus>('/auth/status', undefined, { silent: true }),
  setup: (b: { email: string; password: string; name: string }) => post<LoginResponse>('/auth/setup', b, { silent: true }),
  login: (b: { email: string; password: string; deviceType?: string; deviceName?: string }) =>
    post<LoginResponse>('/auth/login', b, { silent: true }),
  logout: () => post<void>('/auth/logout', undefined, { silent: true }),
  oauthAuthorize: (redirectUri: string) => get<{ url: string }>('/auth/oauth/authorize', { redirectUri }, { silent: true }),
  oauthCallback: (url: string) => post<LoginResponse>('/auth/oauth/callback', { url }, { silent: true }),
  unlockSession: (pin: string) => post<{ unlockedUntil: IsoDate }>('/auth/session/unlock', { pin }),
  lockSession: () => post<void>('/auth/session/lock'),
  session: () => get<SessionInfo>('/auth/session'),
  sessions: () => get<AuthSessionDto[]>('/sessions'),
  deleteSession: (id: Uuid) => del<void>(`/sessions/${id}`),
  deleteOtherSessions: () => del<void>('/sessions'),

  // Users
  me: (silent = false) => get<UserDto>('/users/me', undefined, { silent }),
  updateMe: (b: { name?: string; email?: string; password?: string; currentPassword?: string }) => put<UserDto>('/users/me', b),
  setPin: (b: { pin: string; currentPin?: string }) => put<void>('/users/me/pin', b),
  resetPin: (password?: string) => del<void>('/users/me/pin', { password }),
  preferences: () => get<UserPreferences>('/users/me/preferences'),
  updatePreferences: (p: DeepPartial<UserPreferences>) => put<UserPreferences>('/users/me/preferences', p),
  users: () => get<UserSummary[]>('/users'),
  apiKeys: () => get<ApiKeyDto[]>('/api-keys'),
  createApiKey: (name: string) => post<ApiKeyCreated>('/api-keys', { name }),
  deleteApiKey: (id: Uuid) => del<void>(`/api-keys/${id}`),

  // Admin
  adminUsers: () => get<UserDto[]>('/admin/users'),
  adminCreateUser: (b: { email: string; password: string; name: string; isAdmin?: boolean; quotaBytes?: number | null }) =>
    post<UserDto>('/admin/users', b),
  adminUpdateUser: (
    id: Uuid,
    b: { name?: string; email?: string; password?: string; isAdmin?: boolean; quotaBytes?: number | null },
  ) => put<UserDto>(`/admin/users/${id}`, b),
  adminDeleteUser: (id: Uuid) => del<void>(`/admin/users/${id}`),
  getConfig: () => get<SystemConfig>('/admin/config'),
  putConfig: (c: DeepPartial<SystemConfig>) => put<SystemConfig>('/admin/config', c),
  testAccel: (accel: string, accelDevice: string) =>
    post<{ ok: boolean; encoder?: string; error?: string; devices: string[] }>('/admin/video/accel-test', { accel, accelDevice }),
  backups: () => get<BackupDto[]>('/admin/backups'),
  createBackup: () => post<void>('/admin/backups'),
  deleteBackup: (name: string) => del<void>(`/admin/backups/${encodeURIComponent(name)}`),
  jobs: () => get<Record<string, QueueStatus>>('/jobs'),
  jobCommand: (queue: string, command: JobCommand, force = false) => post<void>(`/jobs/${queue}`, { command, force }),

  // Server
  ping: () => get<{ res: string }>('/server/ping', undefined, { silent: true }),
  serverInfo: () => get<ServerInfo>('/server/info', undefined, { silent: true }),
  serverStatistics: () => get<ServerStatistics>('/server/statistics'),

  // Assets
  bulkUploadCheck: (assets: { id: string; checksum: string }[]) =>
    post<BulkUploadCheckResult>('/assets/bulk-upload-check', { assets }),
  assetsExist: (deviceId: string, deviceAssetIds: string[]) =>
    post<{ existingIds: string[] }>('/assets/exist', { deviceId, deviceAssetIds }),
  asset: (id: Uuid, share?: ShareAccess | null) =>
    get<AssetDto>(`/assets/${id}`, shareQuery(share), { silent: !!share }),
  updateAsset: (id: Uuid, b: AssetUpdate) => put<AssetDto>(`/assets/${id}`, b),
  updateAssets: (b: AssetBulkUpdate) => put<void>('/assets', b),
  deleteAssets: (ids: Uuid[], force = false) => del<void>('/assets', { ids, force }),
  getEdits: (id: Uuid) => get<EditsDto | null>(`/assets/${id}/edits`),
  putEdits: (id: Uuid, edits: EditsDto, saveAsCopy = false) =>
    put<AssetDto>(`/assets/${id}/edits`, saveAsCopy ? { ...edits, saveAsCopy: true } : edits),
  revertEdits: (id: Uuid) => del<AssetDto>(`/assets/${id}/edits`),
  castUrl: (id: Uuid) => post<CastUrl>(`/assets/${id}/cast-url`),
  castDevices: (refresh = false) => get<CastDevice[]>('/cast/devices', refresh ? { refresh: true } : undefined),
  castAddDevice: (location: string) => post<CastDevice>('/cast/devices', { location }),
  castPlay: (deviceId: string, assetId: Uuid) =>
    post<{ device: string }>(`/cast/devices/${encodeURIComponent(deviceId)}/play`, { assetId }),
  castStop: (deviceId: string) => post<{ state: string }>(`/cast/devices/${encodeURIComponent(deviceId)}/stop`),
  assetStatistics: () => get<AssetStatistics>('/assets/statistics'),
  randomAssets: (count = 10) => get<AssetDto[]>('/assets/random', { count }),

  // Trash
  trash: () => get<AssetDto[]>('/trash'),
  restoreTrash: (ids?: Uuid[]) => post<void>('/trash/restore', ids ? { ids } : {}),
  emptyTrash: () => post<void>('/trash/empty'),

  // Timeline
  buckets: (f: TimelineFilters = {}) => get<TimeBucket[]>('/timeline/buckets', { ...f }),
  bucket: (timeBucket: string, f: TimelineFilters = {}) => get<AssetDto[]>('/timeline/bucket', { timeBucket, ...f }),

  // Albums
  albums: (shared?: boolean) => get<AlbumDto[]>('/albums', { shared }),
  createAlbum: (b: {
    albumName: string;
    description?: string;
    assetIds?: Uuid[];
    albumUsers?: { userId: Uuid; role: AlbumRole }[];
  }) => post<AlbumDto>('/albums', b),
  album: (id: Uuid) => get<AlbumDto>(`/albums/${id}`),
  updateAlbum: (
    id: Uuid,
    b: { albumName?: string; description?: string; albumThumbnailAssetId?: Uuid; order?: 'asc' | 'desc' },
  ) => patch<AlbumDto>(`/albums/${id}`, b),
  deleteAlbum: (id: Uuid) => del<void>(`/albums/${id}`),
  addAlbumAssets: (id: Uuid, ids: Uuid[]) => put<BulkIdResult[]>(`/albums/${id}/assets`, { ids }),
  removeAlbumAssets: (id: Uuid, ids: Uuid[]) => del<BulkIdResult[]>(`/albums/${id}/assets`, { ids }),
  addAlbumUsers: (id: Uuid, albumUsers: { userId: Uuid; role: AlbumRole }[]) =>
    put<AlbumDto>(`/albums/${id}/users`, { albumUsers }),
  removeAlbumUser: (id: Uuid, userId: Uuid | 'me') => del<void>(`/albums/${id}/users/${userId}`),
  activities: (albumId: Uuid) => get<ActivityDto[]>(`/albums/${albumId}/activities`),
  addActivity: (albumId: Uuid, b: { type: 'like' | 'comment'; comment?: string; assetId?: Uuid }) =>
    post<ActivityDto>(`/albums/${albumId}/activities`, b),
  deleteActivity: (id: Uuid) => del<void>(`/activities/${id}`),

  // Tags
  tags: () => get<TagDto[]>('/tags'),
  createTag: (value: string, color?: string) => post<TagDto>('/tags', { value, color }),
  updateTag: (id: Uuid, color: string) => put<TagDto>(`/tags/${id}`, { color }),
  deleteTag: (id: Uuid) => del<void>(`/tags/${id}`),
  tagAssets: (id: Uuid, ids: Uuid[]) => put<void>(`/tags/${id}/assets`, { ids }),
  untagAssets: (id: Uuid, ids: Uuid[]) => del<void>(`/tags/${id}/assets`, { ids }),

  // Map
  mapMarkers: (q: { isFavorite?: boolean; withPartners?: boolean; fileCreatedAfter?: string; fileCreatedBefore?: string } = {}) =>
    get<MapMarker[]>('/map/markers', { ...q }),

  // Memories
  memories: (date?: string) => get<Memory[]>('/memories', { date }),

  // People
  people: (withHidden = false) => get<PeopleResponse>('/people', { withHidden }),
  person: (id: Uuid) => get<PersonDto>(`/people/${id}`),
  updatePerson: (id: Uuid, b: { name?: string; birthDate?: string | null; isHidden?: boolean; featureFaceId?: Uuid }) =>
    put<PersonDto>(`/people/${id}`, b),
  personAssets: (id: Uuid) => get<AssetDto[]>(`/people/${id}/assets`),
  mergePeople: (id: Uuid, ids: Uuid[]) => post<PersonDto>(`/people/${id}/merge`, { ids }),
  faces: (assetId: Uuid) => get<FaceDto[]>('/faces', { assetId }),
  reassignFace: (id: Uuid, personId: Uuid | null, createPerson = false) =>
    put<FaceDto>(`/faces/${id}`, createPerson ? { personId: null, createPerson: true } : { personId }),

  // Search
  smartSearch: (b: SmartSearch) => post<SearchResponse>('/search/smart', b),
  metadataSearch: (b: MetadataSearch) => post<SearchResponse>('/search/metadata', b),
  suggestions: (type: SuggestionType, q = '') => get<string[]>('/search/suggestions', { type, q }),
  explore: () => get<ExploreResponse>('/search/explore'),

  // Duplicates
  duplicates: () => get<DuplicateGroup[]>('/duplicates'),
  resolveDuplicate: (id: Uuid, keepAssetIds: Uuid[], trashAssetIds: Uuid[]) =>
    post<void>(`/duplicates/${id}/resolve`, { keepAssetIds, trashAssetIds }),
  dismissDuplicate: (id: Uuid) => del<void>(`/duplicates/${id}`),

  // Libraries
  libraries: () => get<LibraryDto[]>('/libraries'),
  createLibrary: (b: LibraryCreate) => post<LibraryDto>('/libraries', b),
  updateLibrary: (id: Uuid, b: Partial<LibraryCreate>) => put<LibraryDto>(`/libraries/${id}`, b),
  deleteLibrary: (id: Uuid) => del<void>(`/libraries/${id}`),
  scanLibrary: (id: Uuid) => post<void>(`/libraries/${id}/scan`),

  // Partners
  partners: (direction: 'shared-by' | 'shared-with') => get<PartnerDto[]>('/partners', { direction }),
  addPartner: (userId: Uuid) => post<PartnerDto>(`/partners/${userId}`),
  updatePartner: (userId: Uuid, inTimeline: boolean) => put<PartnerDto>(`/partners/${userId}`, { inTimeline }),
  removePartner: (userId: Uuid) => del<void>(`/partners/${userId}`),

  // Shared links
  sharedLinks: () => get<SharedLinkDto[]>('/shared-links'),
  createSharedLink: (b: SharedLinkCreate) => post<SharedLinkDto>('/shared-links', b),
  updateSharedLink: (id: Uuid, b: Partial<Omit<SharedLinkCreate, 'type'>>) => patch<SharedLinkDto>(`/shared-links/${id}`, b),
  deleteSharedLink: (id: Uuid) => del<void>(`/shared-links/${id}`),
  sharedLinkMe: (share: ShareAccess) => get<SharedLinkMe>('/shared-links/me', shareQuery(share), { silent: true }),
  sharedLinkLogin: (key: string, password: string) =>
    post<{ shareToken: string }>('/shared-links/me/login', { key, password }, { silent: true }),
};

// ───────────────────────────── Upload (XHR for progress) ─────────────────────────────

export interface UploadParams {
  file: File;
  checksum?: string;
  deviceAssetId: string;
  deviceId: string;
  isFavorite?: boolean;
  visibility?: Visibility;
  share?: ShareAccess | null;
  onProgress?: (fraction: number) => void;
  signal?: AbortSignal;
}

export function uploadAsset(p: UploadParams): Promise<UploadResult> {
  return new Promise((resolve, reject) => {
    const form = new FormData();
    const modified = new Date(p.file.lastModified || Date.now()).toISOString();
    form.append('deviceAssetId', p.deviceAssetId);
    form.append('deviceId', p.deviceId);
    form.append('fileCreatedAt', modified);
    form.append('fileModifiedAt', modified);
    form.append('isFavorite', p.isFavorite ? 'true' : 'false');
    if (p.visibility) form.append('visibility', p.visibility);
    form.append('assetData', p.file, p.file.name);

    const xhr = new XMLHttpRequest();
    xhr.open('POST', `/api/assets${qs(shareQuery(p.share))}`);
    xhr.withCredentials = true;
    xhr.setRequestHeader('Accept', 'application/json');
    if (token) xhr.setRequestHeader('Authorization', `Bearer ${token}`);
    if (p.checksum) xhr.setRequestHeader('x-checksum', p.checksum);
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) p.onProgress?.(e.loaded / e.total);
    };
    xhr.onload = () => {
      let body: unknown = null;
      try {
        body = JSON.parse(xhr.responseText);
      } catch {
        body = null;
      }
      if (xhr.status >= 200 && xhr.status < 300) resolve(body as UploadResult);
      else reject(new ApiError(xhr.status, body as ApiErrorBody | null, `${xhr.status} ${xhr.statusText}`));
    };
    xhr.onerror = () => reject(new ApiError(0, null, 'Network error'));
    xhr.onabort = () => reject(new ApiError(0, null, 'aborted'));
    p.signal?.addEventListener('abort', () => xhr.abort());
    xhr.send(form);
  });
}
