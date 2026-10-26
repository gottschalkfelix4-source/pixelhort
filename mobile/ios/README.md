# Pixelhort for iOS

Native SwiftUI client for the self-hosted Pixelhort server (REST contract: [`docs/API.md`](../../docs/API.md)).

* iOS 17+, Swift 5 language mode, SwiftUI + Observation
* No third-party dependencies — Apple frameworks only (Photos, AVKit, MapKit, BackgroundTasks,
  AuthenticationServices, LocalAuthentication, CryptoKit, Security, Network), so the project builds offline.
* UI languages: German (development language) and English (`Resources/{de,en}.lproj`).

## Build

Requirements: macOS with **Xcode 16** or newer (iOS 18 SDK; deployment target stays iOS 17), [XcodeGen](https://github.com/yonaskolb/XcodeGen).

```sh
brew install xcodegen
cd mobile/ios
xcodegen generate          # creates Pixelhort.xcodeproj from project.yml (and refreshes Pixelhort/Info.plist)
open Pixelhort.xcodeproj
```

In Xcode select the *Pixelhort* target → *Signing & Capabilities* → choose your team
(or set `DEVELOPMENT_TEAM` in `project.yml`), then run on a device or simulator.

Command line build (simulator, no signing):

```sh
xcodebuild -project Pixelhort.xcodeproj -scheme Pixelhort \
  -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO build
```

### Server

Start a Pixelhort server (default port **2342**) and enter its address on first launch, e.g.
`http://192.168.1.10:2342`. Plain `http` is allowed (`NSAllowsArbitraryLoads`) because self-hosted
servers usually live on the LAN; iOS will ask for *local network* permission.

For SSO, configure the server's OAuth provider with the mobile redirect URI
`app.pixelhort:/oauth-callback` (`PH_OAUTH_MOBILE_REDIRECT_URI`). The app opens the IdP with
`ASWebAuthenticationSession` and posts the callback URL to `POST /api/auth/oauth/callback`.

## Features

| Area | Implementation |
|---|---|
| Onboarding | Server URL validation (`/api/server/ping`, `/api/auth/status`), email/password login (`deviceType: "ios"`), OAuth via `ASWebAuthenticationSession`. Access token + device id in the Keychain (`AfterFirstUnlockThisDeviceOnly`, readable by background tasks). |
| **Automatic backup** | `Backup/` – all photos or selected `PHAssetCollection`s, Wi-Fi only, include videos, background backup. Per asset: export original via `PHAssetResourceManager` → SHA-1 (CryptoKit, streamed) → `POST /api/assets/bulk-upload-check` → multipart body written to a file → background `URLSession` upload task (`POST /api/assets`, header `x-checksum`). Live Photos: paired video uploaded first (`visibility=hidden`, `deviceAssetId = <localIdentifier>#live-video`), then the photo with `livePhotoVideoId`. State persisted as JSON (`UploadStateStore`, keyed by `localIdentifier` + checksum + modification date). After a reinstall `POST /api/assets/exist` avoids re-hashing known assets. Triggers: app becomes active, `PHPhotoLibraryChangeObserver` (debounced), `BGAppRefreshTask` + `BGProcessingTask`, background-session relaunch events. Progress screen with current file, counts, recent uploads and errors. |
| Timeline | `/api/timeline/buckets` + lazily `/api/timeline/bucket` per month, grouped by day (UTC wall clock), `LazyVGrid`, configurable column count, authenticated thumbnails with memory (`NSCache`) + disk cache. Multi-select: share, favorite, add to album, shared link, archive, locked folder, delete (context dependent: restore/delete permanently in trash, remove from album …). |
| Viewer | Horizontal paging (`scrollTargetBehavior(.paging)`), pinch/double-tap zoom, swipe down to close, video via `AVPlayer` with `AVURLAsset` option `AVURLAssetHTTPHeaderFieldsKey` (Bearer token), Live/Motion Photo playback on long-press, info sheet (EXIF, map, people, tags), favorite/delete/share/save to Photos (Live Photos saved as pair), archive/locked folder/add to album/shared link. |
| Casting | AirPlay route picker (`AVRoutePickerView`) for videos. "Auf Fernseher streamen" requests `POST /api/assets/{id}/cast-url`; the castToken URL is played by `AVPlayer` (AirPlay receivers fetch it directly – no auth header needed) or shared/copied for other receivers. |
| Albums | Own / shared albums, detail (grouped timeline with `albumId` filter), create, rename, delete/leave, add assets, share with users (viewer/editor). |
| Library | Favorites, Archive, Trash (restore all / empty), Locked folder, People (rename, hide), Map, Shared links, Partners. |
| Search & explore | Smart search (CLIP, paged), explore people / places / things (`/api/search/explore`, metadata search for places/objects). |
| Map | MapKit (iOS 17 `Map` API) with annotations from `/api/map/markers`, grid clustering depending on zoom, favorites filter. |
| Memories | "On this day" carousel from `/api/memories` on top of the timeline. |
| Shared links | Create (description, password, expiry, download/upload/metadata), list with `ShareLink`, copy, delete. |
| Partners | Share my library, accept/hide partner libraries in the timeline (`inTimeline`), timeline `withPartners` toggle. |
| Locked folder | PIN setup (`PUT /api/users/me/pin`), unlock (`POST /api/auth/session/unlock`), auto-lock on leave. Face ID/Touch ID (LocalAuthentication) is a convenience gate releasing the PIN stored in the Keychain. |
| Settings | Account (name, storage quota), backup options, grid columns, partner photos, server/app info, image cache, logout. |

## Source layout

```
mobile/ios/
├── project.yml                     XcodeGen spec (bundle id app.pixelhort, Info.plist keys)
└── Pixelhort/
    ├── Info.plist                  generated by XcodeGen from project.yml (kept in sync)
    ├── App/                        @main app, AppDelegate (BGTasks, background URLSession), session state, tabs
    ├── Core/                       Foundation-only: models, API client, multipart writer, upload state store
    │   ├── Models/
    │   ├── Networking/
    │   └── Persistence/
    ├── Platform/                   Keychain, biometrics, SHA-1, image cache, Photos saver, network monitor
    ├── Backup/                     asset export, background uploader, backup service, BGTask scheduler
    ├── Shared/                     reusable views + UIKit bridges (share sheet, AirPlay, player layer)
    ├── Features/                   Onboarding, Timeline, Viewer, Albums, Library, Search, Map, Sharing, Locked, Settings
    └── Resources/                  de/en Localizable.strings + InfoPlist.strings, asset catalog
```

## Verification status

`Core/` (models, JSON coding, API client incl. all endpoint wrappers, multipart writer, upload state
store) has no Apple-only dependencies and is type-checked **and unit-tested** on Linux with the
official Swift image:

```sh
sh Tools/check-core.sh     # swiftc -typecheck (Swift 5 + Swift 6 mode) + Tools/CoreSmokeTest
```

The UI and PhotoKit/AVKit/MapKit code can only be compiled with Xcode on macOS.

## Optional: Google Cast (Chromecast)

Not included, to keep the project dependency-free. To add it:

1. Add the Google Cast SDK via Swift Package Manager / CocoaPods (`google-cast-sdk`).
2. Add `NSBonjourServices` (`_googlecast._tcp`, `_<APP_ID>._googlecast._tcp`) to the Info.plist.
3. Initialise `GCKCastContext` in `AppDelegate`, show a `GCKUICastButton` in `CastSheet`, and load
   `CastURLResponse.url` (video, `mimeType`) or `imageUrl` (photo) as `GCKMediaInformation` – these
   URLs already carry a `castToken`, so the receiver needs no credentials.

## App icon

`Resources/Assets.xcassets/AppIcon.appiconset` contains an empty 1024 pt slot – drop a
1024×1024 PNG in Xcode before shipping.
