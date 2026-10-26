# Pixelhort REST API (v1)

This document is the **contract** between the server (`crates/server`), the web
app (`web/`), the CLI (`crates/cli`) and the mobile apps (`mobile/android`,
`mobile/ios`). The ML service API is documented at the end.

## Conventions

* Base path: `/api`. Default server port: **2342**.
* JSON everywhere, **camelCase** keys. Timestamps are RFC 3339 strings in UTC
  (`2024-05-01T12:00:00Z`). IDs are UUID strings.
* Authentication (any of):
  * `Authorization: Bearer <accessToken>` (login session token)
  * `x-api-key: <apiKey>`
  * cookie `ph_session=<accessToken>` (set by `/api/auth/login` for the web app)
  * public share access: query `?key=<sharedLinkKey>` (and `&shareToken=<token>` for password protected links)
  * cast access: query `?castToken=<token>` (only asset media endpoints)
* Errors: HTTP status + body `{"statusCode":400,"error":"Bad Request","message":"human readable"}`.
* `401` = not logged in, `403` = forbidden (also: locked folder not unlocked, message `"locked"`).

---

## Auth

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/auth/status` | – | `{"setupRequired":bool,"oauthEnabled":bool,"oauthButtonText":"Login with SSO","passwordLoginEnabled":bool}` |
| POST | `/api/auth/setup` | `{"email","password","name"}` – only works while no user exists; creates admin | `LoginResponse` |
| POST | `/api/auth/login` | `{"email","password","deviceType?":"web/android/ios/cli","deviceName?"}` | `LoginResponse` (also sets `ph_session` cookie) |
| POST | `/api/auth/logout` | – | `204` |
| GET | `/api/auth/oauth/authorize?redirectUri=<uri>` | – | `{"url":"https://idp/..."}` – open this URL in browser |
| POST | `/api/auth/oauth/callback` | `{"url":"<full redirect url incl. ?code=&state=>"}` | `LoginResponse` |
| POST | `/api/auth/session/unlock` | `{"pin":"1234"}` | `{"unlockedUntil":"..."}` (locked folder access for 15 min) |
| POST | `/api/auth/session/lock` | – | `204` |
| GET | `/api/auth/session` | – | `{"id","unlockedUntil":null\|"...","hasPin":bool}` |

`LoginResponse`:
```json
{"accessToken":"opaque-string","user": UserDto}
```
`UserDto`:
```json
{"id":"uuid","email":"a@b.c","name":"Alice","isAdmin":true,"quotaBytes":null,"usageBytes":12345,
 "hasPin":false,"oauthId":null,"createdAt":"...","updatedAt":"..."}
```

OAuth redirect URIs: web uses `<publicUrl>/auth/oauth-callback`, mobile uses
`app.pixelhort:/oauth-callback` (configurable via `PH_OAUTH_MOBILE_REDIRECT_URI`).

## Users & API keys

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/users/me` | – | `UserDto` |
| PUT | `/api/users/me` | `{"name?","email?","password?","currentPassword?"}` | `UserDto` |
| PUT | `/api/users/me/pin` | `{"pin":"1234","currentPin?":"0000"}` (4–8 digits) | `204` |
| GET | `/api/users` | – | `[UserSummary]` – all users (for sharing pickers) |
| GET | `/api/api-keys` | – | `[{"id","name","createdAt"}]` |
| POST | `/api/api-keys` | `{"name"}` | `{"id","name","createdAt","secret":"phk_..."}` (secret shown once) |
| DELETE | `/api/api-keys/{id}` | – | `204` |

`UserSummary`: `{"id","name","email"}`

### Admin (requires `isAdmin`)

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/admin/users` | – | `[UserDto]` |
| POST | `/api/admin/users` | `{"email","password","name","isAdmin?":false,"quotaBytes?":null}` | `UserDto` |
| PUT | `/api/admin/users/{id}` | `{"name?","email?","password?","isAdmin?","quotaBytes?"}` | `UserDto` |
| DELETE | `/api/admin/users/{id}` | – | `204` (deletes user and all their assets) |
| GET | `/api/admin/config` | – | `SystemConfig` (see below) |
| PUT | `/api/admin/config` | `SystemConfig` (partial allowed) | `SystemConfig` |
| GET | `/api/admin/backups` | – | `[{"name":"pixelhort-2024...sql.gz","size":123,"createdAt":"..."}]` |
| POST | `/api/admin/backups` | – | `202` (starts a database backup job) |
| GET | `/api/admin/backups/{name}` | – | file download |
| DELETE | `/api/admin/backups/{name}` | – | `204` |
| GET | `/api/jobs` | – | `{"<queue>":{"active":0,"waiting":3,"failed":1,"completed":10,"paused":false}}` |
| POST | `/api/jobs/{queue}` | `{"command":"start"\|"pause"\|"resume"\|"clear-failed","force?":false}` | `204` |

Queues: `metadata`, `thumbnails`, `video`, `smart-search`, `faces`, `facial-recognition`,
`ocr`, `objects`, `duplicates`, `library`, `backup`.
`start` enqueues all assets missing that step (or all assets with `force:true`).

`SystemConfig`:
```json
{
 "ml": {"enabled":true,"url":"http://ml:3003","clipModel":"ViT-B-32__openai",
        "faceMinScore":0.7,"faceMaxDistance":0.5,"faceMinFaces":3,
        "duplicateMaxDistance":0.01,"ocrEnabled":true,"objectsEnabled":true},
 "backup": {"enabled":true,"cron":"0 0 2 * * *","keep":14},
 "video": {"targetResolution":720,"crf":23,"preset":"veryfast"},
 "trash": {"days":30},
 "reverseGeocoding": {"enabled":true},
 "passwordLogin": {"enabled":true},
 "oauth": {"enabled":false,"issuerUrl":"","clientId":"","clientSecret":"","scope":"openid email profile",
           "buttonText":"Login with SSO","autoRegister":true,"mobileRedirectUri":"app.pixelhort:/oauth-callback"}
}
```

## Server

| Method | Path | Response |
|---|---|---|
| GET | `/api/server/ping` | `{"res":"pong"}` (no auth) |
| GET | `/api/server/info` | `{"version":"0.1.0","features":{"smartSearch":bool,"faces":bool,"ocr":bool,"objects":bool,"oauth":bool,"reverseGeocoding":bool,"cast":true},"publicUrl":"..."}` (no auth) |
| GET | `/api/server/statistics` | admin: `{"photos":1,"videos":2,"usageBytes":3,"users":[{"userId","name","photos","videos","usageBytes"}]}` |

Prometheus metrics: `GET /metrics` on the metrics listener (default port **9464**).

## Assets

`AssetDto`:
```json
{
 "id":"uuid","ownerId":"uuid","libraryId":null,
 "deviceAssetId":"local-id","deviceId":"device-uuid",
 "type":"IMAGE",                      // IMAGE | VIDEO
 "originalFileName":"IMG_0001.HEIC","originalMimeType":"image/heic",
 "checksum":"sha1 hex","fileSize":123456,
 "fileCreatedAt":"...","fileModifiedAt":"...",
 "localDateTime":"2024-05-01T12:00:00Z",   // capture time (wall clock, encoded as UTC) – used for grouping/sorting
 "createdAt":"...","updatedAt":"...",
 "width":4032,"height":3024,"duration":null,  // duration in seconds (float) for videos
 "isFavorite":false,
 "visibility":"timeline",            // timeline | archive | hidden | locked
 "isTrashed":false,"isOffline":false,
 "livePhotoVideoId":null,            // id of the hidden video part of a live/motion photo
 "isMotionPhoto":false,
 "projectionType":null,              // null | "EQUIRECTANGULAR" (360° photo)
 "isPanorama":false,"isRaw":false,
 "thumbhash":"base64 or null",
 "rating":null,"description":"",
 "hasEdits":false,"duplicateId":null,
 "exif": ExifDto | null,             // always present on GET /api/assets/{id}; lists may send null
 "tags":[{"id","value","color"}],    // filled on detail endpoint, [] in lists
 "people":[{"id","name"}]            // filled on detail endpoint, [] in lists
}
```
`ExifDto`: `{"make","model","lensModel","fNumber","exposureTime","iso","focalLength","latitude","longitude","altitude","city","state","country","orientation","dateTimeOriginal","timeZone","projectionType","description"}` (all nullable).

| Method | Path | Body | Response |
|---|---|---|---|
| POST | `/api/assets` | **multipart**: `assetData` (file, required), `deviceAssetId`, `deviceId`, `fileCreatedAt`, `fileModifiedAt`, `isFavorite` ("true"/"false"), `livePhotoVideoId` (optional), `visibility` (optional), `sidecarData` (optional .xmp file). Optional header `x-checksum: <sha1 hex>` for an early duplicate check. With `?key=` uploads into a shared album (if allowed). | `201 {"id","status":"created"}` or `200 {"id","status":"duplicate"}` |
| POST | `/api/assets/bulk-upload-check` | `{"assets":[{"id":"client-ref","checksum":"sha1 hex"}]}` | `{"results":[{"id":"client-ref","action":"accept"\|"reject","reason":"duplicate","assetId":"uuid"}]}` |
| POST | `/api/assets/exist` | `{"deviceId","deviceAssetIds":["..."]}` | `{"existingIds":["..."]}` |
| GET | `/api/assets/{id}` | – | `AssetDto` (full) |
| PUT | `/api/assets/{id}` | `{"isFavorite?","visibility?","description?","rating?","dateTimeOriginal?","latitude?","longitude?"}` | `AssetDto` |
| PUT | `/api/assets` | `{"ids":[...],"isFavorite?","visibility?","rating?"}` | `204` |
| DELETE | `/api/assets` | `{"ids":[...],"force?":false}` – moves to trash (`force` = delete permanently) | `204` |
| GET | `/api/assets/{id}/original` | `?edited=true` renders edits into full resolution JPEG | file (supports `Range`) |
| GET | `/api/assets/{id}/thumbnail` | `?size=thumbnail` (≈250px short edge JPEG, default) or `preview` (≈1440px long edge JPEG) | image |
| GET | `/api/assets/{id}/video` | – | MP4 for playback (transcoded if needed, else original; supports `Range`) |
| GET | `/api/assets/{id}/edits` | – | `EditsDto` or `null` |
| PUT | `/api/assets/{id}/edits` | `EditsDto` (+ optional `"saveAsCopy":true` → returns the new asset) | `AssetDto` |
| DELETE | `/api/assets/{id}/edits` | – | `AssetDto` (reverts to original) |
| POST | `/api/assets/{id}/cast-url` | – | `{"url":"http(s)://.../api/assets/{id}/video?castToken=...","mimeType":"video/mp4","imageUrl":"...thumbnail?size=preview&castToken=..."}` (valid 4h) |
| GET | `/api/assets/statistics` | – | `{"images":1,"videos":2,"total":3}` |
| GET | `/api/assets/random?count=10` | – | `[AssetDto]` |

`EditsDto` (non-destructive, original stays untouched):
```json
{"crop":{"x":0.1,"y":0.1,"width":0.8,"height":0.8},   // normalized 0..1 relative to the rotated image, nullable
 "rotate":90,                // 0 | 90 | 180 | 270 (clockwise, applied before crop)
 "flipHorizontal":false,"flipVertical":false,
 "brightness":0,"contrast":0,"saturation":0,           // -100..100
 "filter":null}              // null | "bw" | "sepia" | "warm" | "cool" | "vintage"
```

### Trash
| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/trash` | – | `[AssetDto]` |
| POST | `/api/trash/restore` | `{"ids?":[...]}` (omit = restore all) | `204` |
| POST | `/api/trash/empty` | – | `204` |

## Timeline

Common filter query parameters: `visibility` (default `timeline`; `archive`,
`locked` – requires unlocked session), `isFavorite=true`, `albumId`, `personId`,
`tagId`, `withPartners=true` (include partners with `inTimeline`), `isTrashed=true`, `order=desc|asc`.

| Method | Path | Response |
|---|---|---|
| GET | `/api/timeline/buckets?<filters>` | `[{"timeBucket":"2024-05-01","count":42}]` – one bucket per month, newest first |
| GET | `/api/timeline/bucket?timeBucket=2024-05-01&<filters>` | `[AssetDto]` sorted by `localDateTime` |

## Sync (mobile)

| Method | Path | Response |
|---|---|---|
| GET | `/api/sync/delta?updatedAfter=<iso>&limit=1000` | `{"upserted":[AssetDto],"deleted":["uuid"],"hasMore":bool,"cursor":"<iso to use as next updatedAfter>"}` |

## Albums

`AlbumDto`:
```json
{"id","ownerId","owner":UserSummary,"albumName","description","albumThumbnailAssetId",
 "assetCount":10,"startDate":"...","endDate":"...","createdAt","updatedAt",
 "shared":bool,"order":"desc",
 "albumUsers":[{"user":UserSummary,"role":"editor"|"viewer"}],
 "assets":[AssetDto]}          // only on GET /api/albums/{id}
```

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/albums?shared=true\|false` | omit = all accessible | `[AlbumDto]` |
| POST | `/api/albums` | `{"albumName","description?","assetIds?":[],"albumUsers?":[{"userId","role"}]}` | `AlbumDto` |
| GET | `/api/albums/{id}` | – | `AlbumDto` incl. assets |
| PATCH | `/api/albums/{id}` | `{"albumName?","description?","albumThumbnailAssetId?","order?"}` | `AlbumDto` |
| DELETE | `/api/albums/{id}` | – | `204` |
| PUT | `/api/albums/{id}/assets` | `{"ids":[...]}` | `[{"id","success":bool,"error?":"duplicate"\|"no_permission"}]` |
| DELETE | `/api/albums/{id}/assets` | `{"ids":[...]}` | same as above |
| PUT | `/api/albums/{id}/users` | `{"albumUsers":[{"userId","role"}]}` | `AlbumDto` |
| DELETE | `/api/albums/{id}/users/{userId}` | – (`me` allowed = leave album) | `204` |

## Tags

`TagDto`: `{"id","value":"Holiday/Italy","name":"Italy","color":"#ff0000","parentId":null,"assetCount":3}`

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/tags` | – | `[TagDto]` |
| POST | `/api/tags` | `{"value":"Holiday/Italy","color?"}` (parents created automatically) | `TagDto` |
| PUT | `/api/tags/{id}` | `{"color?"}` | `TagDto` |
| DELETE | `/api/tags/{id}` | – | `204` |
| PUT | `/api/tags/{id}/assets` | `{"ids":[...]}` | `204` |
| DELETE | `/api/tags/{id}/assets` | `{"ids":[...]}` | `204` |

## Map

| Method | Path | Response |
|---|---|---|
| GET | `/api/map/markers?isFavorite=&withPartners=&fileCreatedAfter=&fileCreatedBefore=` | `[{"id":"assetId","lat":52.5,"lon":13.4,"city":"Berlin","state":"Berlin","country":"Germany"}]` |

## Memories

| Method | Path | Response |
|---|---|---|
| GET | `/api/memories?date=2024-10-26` (default today) | `[{"id":"on-this-day-2019","type":"on_this_day","yearsAgo":5,"year":2019,"title":"5 years ago","assets":[AssetDto]}]` |

## People & faces

`PersonDto`: `{"id","name":"","birthDate":null,"isHidden":false,"faceCount":12,"thumbnailUrl":"/api/people/{id}/thumbnail","updatedAt"}`

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/people?withHidden=false` | – | `{"people":[PersonDto],"total":10,"hidden":2}` |
| GET | `/api/people/{id}` | – | `PersonDto` |
| PUT | `/api/people/{id}` | `{"name?","birthDate?","isHidden?","featureFaceId?"}` | `PersonDto` |
| GET | `/api/people/{id}/thumbnail` | – | JPEG face crop |
| GET | `/api/people/{id}/assets` | – | `[AssetDto]` |
| POST | `/api/people/{id}/merge` | `{"ids":["otherPersonId"]}` | `PersonDto` |
| GET | `/api/faces?assetId=` | – | `[{"id","personId","person":PersonDto\|null,"boundingBoxX1","boundingBoxY1","boundingBoxX2","boundingBoxY2","imageWidth","imageHeight"}]` |
| PUT | `/api/faces/{id}` | `{"personId":"uuid"\|null}` (null + `"createPerson":true` creates a new person) | face object |

## Search

`SearchResponse`: `{"assets":[AssetDto],"nextPage":2|null,"total":123}`

| Method | Path | Body | Response |
|---|---|---|---|
| POST | `/api/search/smart` | `{"query":"dog on the beach","page?":1,"size?":100, <metadata filters>}` | `SearchResponse` (CLIP) |
| POST | `/api/search/metadata` | `{"originalFileName?","text?" (OCR full text),"make?","model?","lensModel?","city?","state?","country?","takenAfter?","takenBefore?","type?":"IMAGE"\|"VIDEO","isFavorite?","personIds?":[],"tagIds?":[],"objects?":["dog"],"isRaw?","isPanorama?","is360?","isMotion?","visibility?","page?","size?"}` | `SearchResponse` |
| GET | `/api/search/suggestions?type=city\|country\|make\|model\|object&q=` | – | `["Berlin","Hamburg"]` |
| GET | `/api/search/explore` | – | `{"places":[{"value":"Berlin","assetId"}],"objects":[{"value":"dog","assetId"}],"people":[PersonDto]}` |

## Duplicates

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/duplicates` | – | `[{"duplicateId","assets":[AssetDto]}]` |
| POST | `/api/duplicates/{duplicateId}/resolve` | `{"keepAssetIds":[...],"trashAssetIds":[...]}` | `204` |
| DELETE | `/api/duplicates/{duplicateId}` | – (dismiss: "not duplicates") | `204` |

## Libraries (external, admin)

`LibraryDto`: `{"id","ownerId","name","importPaths":["/mnt/photos"],"exclusionPatterns":["**/@eaDir/**"],"watch":true,"assetCount":10,"refreshedAt":null,"createdAt"}`

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/libraries` | – | `[LibraryDto]` |
| POST | `/api/libraries` | `{"ownerId","name","importPaths","exclusionPatterns?","watch?"}` | `LibraryDto` |
| PUT | `/api/libraries/{id}` | partial | `LibraryDto` |
| DELETE | `/api/libraries/{id}` | – (removes assets from DB, files untouched) | `204` |
| POST | `/api/libraries/{id}/scan` | – | `202` |

Paths refer to the server's filesystem (mount them read-only into the container).

## Partners

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/partners?direction=shared-by\|shared-with` | – | `[{"user":UserSummary,"inTimeline":bool}]` |
| POST | `/api/partners/{userId}` | – (share *my* library with userId) | `{"user","inTimeline"}` |
| PUT | `/api/partners/{userId}` | `{"inTimeline":true}` (receiver toggles) | `{"user","inTimeline"}` |
| DELETE | `/api/partners/{userId}` | – | `204` |

Partner assets are read-only for the partner, never include `archive`, `hidden` or `locked` assets.

## Shared links (public)

`SharedLinkDto`:
```json
{"id","key":"base64url","type":"ALBUM"|"INDIVIDUAL","albumId":null,"album":AlbumDto|null,
 "assetIds":[...],"description","hasPassword":bool,"expiresAt":null,
 "allowDownload":true,"allowUpload":false,"showMetadata":true,"createdAt","url":"<publicUrl>/s/<key>"}
```

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/shared-links` | – | `[SharedLinkDto]` |
| POST | `/api/shared-links` | `{"type","albumId?","assetIds?","description?","password?","expiresAt?","allowDownload?","allowUpload?","showMetadata?"}` | `SharedLinkDto` |
| PATCH | `/api/shared-links/{id}` | partial (password `""` removes it) | `SharedLinkDto` |
| DELETE | `/api/shared-links/{id}` | – | `204` |
| GET | `/api/shared-links/me?key=K[&shareToken=T]` | – (no login) | `{"link":SharedLinkDto,"assets":[AssetDto]}` or `401 {"message":"password_required"}` |
| POST | `/api/shared-links/me/login` | `{"key","password"}` | `{"shareToken":"..."}` |

Asset media of a share is fetched with the same `?key=K[&shareToken=T]` query
parameters on `/api/assets/{id}/thumbnail|original|video`.

## Casting to TVs (DLNA/UPnP)

Smart TVs and media players that act as DLNA media renderers. The server discovers
them via SSDP and tells them to fetch the media through a signed cast URL
(`PH_PUBLIC_URL`, otherwise the server's LAN address towards the device).

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/cast/devices?refresh=true` | – (refresh forces a new SSDP search, otherwise cached 60 s) | `[{"id","name","manufacturer","model","location"}]` |
| POST | `/api/cast/devices` | `{"location":"http://192.168.1.20:9197/dmr"}` – add a renderer by its UPnP description URL (remembered across restarts) | device |
| POST | `/api/cast/devices/{id}/play` | `{"assetId"}` | `{"device","url","mimeType"}` |
| POST | `/api/cast/devices/{id}/stop` | – | `{"state":"STOPPED"}` |
| GET | `/api/cast/devices/{id}/status` | – | `{"state":"PLAYING"\|"STOPPED"\|"PAUSED_PLAYBACK"\|"TRANSITIONING"}` |

Google Cast (web/Android) and AirPlay (iOS) use `POST /api/assets/{id}/cast-url` instead.

## Activity stream for shared albums (optional, likes/comments)

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/albums/{id}/activities` | – | `[{"id","user":UserSummary,"type":"like"\|"comment","comment","assetId","createdAt"}]` |
| POST | `/api/albums/{id}/activities` | `{"type","comment?","assetId?"}` | activity |
| DELETE | `/api/activities/{id}` | – | `204` |

---

# ML service API (`crates/ml`, default port 3003)

The server talks to it via `PH_ML_URL` (can run on any other machine; GPU
optional). Optional shared secret header `x-ml-key` (`PH_ML_API_KEY`).

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/ping` | – | `pong` (text) |
| GET | `/health` | – | `{"status":"ok","models":{"clip":true,"faces":true,"ocr":true}}` |
| POST | `/predict` | multipart: `image` (JPEG/PNG/WebP), `tasks` = comma list of `clip,faces,ocr,objects`, optional `faceMinScore` (default 0.7) | `PredictResponse` |
| POST | `/clip/text` | `{"text":"a dog"}` | `{"embedding":[512 floats, L2-normalized]}` |

`PredictResponse`:
```json
{"imageWidth":1440,"imageHeight":1080,
 "clip":[512 floats] | null,
 "faces":[{"boundingBox":{"x1":10,"y1":20,"x2":110,"y2":140},"score":0.98,"embedding":[512 floats]}] | null,
 "ocr":{"text":"full text joined by newlines","lines":[{"text":"STOP","score":0.97,"box":[x1,y1,x2,y2,x3,y3,x4,y4]}]} | null,
 "objects":[{"label":"dog","score":0.31}] | null}
```
Bounding boxes are in pixels of the submitted image.
