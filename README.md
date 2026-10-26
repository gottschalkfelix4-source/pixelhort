# Pixelhort

Selbst gehostete Foto- und Videoverwaltung, eine von Grund auf neu geschriebene
Alternative zu Immich. Der Server ist in **Rust** geschrieben, dazu gibt es eine
Web-App (Svelte), eine **CLI** sowie **native Apps für iOS (SwiftUI) und Android
(Kotlin/Compose)**. Machine Learning läuft als eigener Rust-Dienst und kann auf
einen anderen Rechner (z. B. mit GPU) ausgelagert werden.

```
            ┌──────────── Web (Svelte SPA) ────────────┐
 iOS-App ───┤                                          │
 Android ───┼──► pixelhort-server (Rust, axum) ──► PostgreSQL + pgvector
 CLI ───────┘      │  Jobs-Queue, Thumbnails, EXIF,      (Metadaten, Vektoren)
                   │  Video, Backups, /metrics
                   └──HTTP──► pixelhort-ml (Rust, ONNX Runtime)
                              CLIP · Gesichter · OCR · Objekte   ← beliebiger Rechner
```

## Funktionen

| Funktion | Umsetzung |
|---|---|
| Automatisches Backup per App | Android: WorkManager + MediaStore-Trigger. iOS: PhotoKit + BGTasks + Background-URLSession. Duplikatprüfung per SHA-1 (`bulk-upload-check`) |
| Upload per Web/CLI | Drag & Drop in der Web-App, `pixelhort upload <ordner> --album …` |
| Externe Bibliotheken | Ordner read-only indexieren, Ausschlussmuster, Dateisystem-Watcher und stündlicher Rescan, Offline-Erkennung |
| Gesichtserkennung + Gruppierung | SCRFD + ArcFace (InsightFace buffalo_l), inkrementelles Clustering über pgvector (`faceMaxDistance`, `faceMinFaces`) |
| Smart Search (CLIP) | ViT-B-32 Bild-/Text-Embeddings, HNSW-Index, Suche in natürlicher Sprache |
| Objekterkennung & OCR | CLIP-Zero-Shot-Labels; PP-OCR (Erkennung + Latin-Erkennungsmodell), Volltextsuche |
| Duplikaterkennung | Identische Dateien per Checksumme, ähnliche Bilder per CLIP-Distanz, Review-Seite |
| Timeline, Alben, Favoriten, Tags | Monats-Buckets, hierarchische Tags (`Urlaub/Italien`), Import von EXIF/XMP-Keywords |
| Karte | GPS aus EXIF/QuickTime, Offline-Reverse-Geocoding (GeoNames) |
| Memories („An diesem Tag“) | `/api/memories` |
| Live Photos, Motion Photos, 360°, Panoramen, RAW | iOS-Paare über ContentIdentifier oder `livePhotoVideoId`; eingebettetes MP4 aus Google-/Samsung-Motion-Photos wird extrahiert; GPano-Projektion; RAW-Vorschau über exiftool; HEIC/AVIF über libheif |
| Bildeditor | Nicht-destruktiv: Zuschneiden, Drehen, Spiegeln, Helligkeit, Kontrast, Sättigung, Filter, optional als Kopie speichern |
| Gesperrter Ordner | PIN (argon2), Session-Entsperrung für 15 Min.; gesperrte Assets verschwinden aus Suche, Alben, Freigaben, Partnern |
| Mehrere Benutzer | Admin-Verwaltung, Speicherkontingente, API-Keys |
| Geteilte Alben, Partner, öffentliche Links | Rollen Editor/Betrachter, Kommentare/Likes, Partner-Timeline, Links mit Passwort/Ablauf/Download/Upload |
| Casting auf TV | Signierte, kurzlebige Cast-URLs; Google Cast (Web, Android), AirPlay (iOS) |
| OAuth-Login | OpenID Connect mit PKCE, Auto-Registrierung, Web- und Mobile-Redirect |
| Automatisches DB-Backup | `pg_dump` + gzip nach Cron-Ausdruck, Aufbewahrung der letzten N Backups, Download im Admin-Bereich |
| REST-API & Prometheus | Dokumentiert in [docs/API.md](docs/API.md); Metriken auf Port 9464 (`/metrics`), ML-Dienst ebenfalls mit `/metrics` |
| ML auslagerbar | `PH_ML_URL=http://gpu-rechner:3003`, optional mit `PH_ML_API_KEY` gesichert |

## Schnellstart (Docker)

```bash
cp .env.example .env    # Passwörter und Pfade anpassen
docker compose up -d --build
```

Danach `http://<host>:2342` öffnen. Das erste Konto wird Administrator.

### Unraid

Templates für Server, ML-Dienst und Datenbank liegen in [unraid/](unraid/README.md)
(eigenes Docker-Netzwerk `pixelhort`, `PUID`/`PGID` 99:100). Die Images werden per
GitHub Actions nach `ghcr.io/gottschalkfelix4-source/pixelhort-*` veröffentlicht.

### ML auf einem anderen Rechner

Auf dem ML-Rechner:

```bash
docker compose up -d ml          # oder: docker run -p 3003:3003 -v ml-cache:/cache pixelhort-ml
```

Auf dem Server in `.env` `PH_ML_URL=http://<ml-rechner>:3003` setzen, oder die
URL im Admin-Bereich unter Einstellungen → ML eintragen. GPU-Build:
`docker build -f docker/ml.Dockerfile --build-arg FEATURES=cuda …` (siehe
[crates/ml/README.md](crates/ml/README.md)).

## Komponenten

| Pfad | Inhalt |
|---|---|
| `crates/server` | API-Server, Job-Queue (Postgres `SKIP LOCKED`), Medienverarbeitung, Scheduler |
| `crates/ml` | ML-Dienst (eigenes Cargo-Projekt), Modelle werden beim ersten Start von Hugging Face geladen |
| `crates/cli` | `pixelhort` CLI (login, upload, albums, info) |
| `crates/common` | Gemeinsame API-Typen |
| `web/` | Web-App (Vite + Svelte 5 + TypeScript) |
| `mobile/android` | Native Android-App (Kotlin, Jetpack Compose) |
| `mobile/ios` | Native iOS-App (SwiftUI, XcodeGen-Projekt) |
| `docs/API.md` | REST-API-Vertrag |
| `tests/e2e.py` | End-to-End-Test gegen einen laufenden Stack |

## Konfiguration (Server)

| Variable | Standard | Bedeutung |
|---|---|---|
| `PH_DATABASE_URL` | `postgres://pixelhort:pixelhort@localhost:5432/pixelhort` | PostgreSQL mit pgvector |
| `PH_DATA_DIR` | `./data` | Originale, Thumbnails, Videos, Backups |
| `PH_LISTEN` / `PH_METRICS_LISTEN` | `0.0.0.0:2342` / `0.0.0.0:9464` (`off` deaktiviert) | Listener |
| `PH_PUBLIC_URL` | – | Externe URL für Freigabelinks und Casting |
| `PH_ML_URL`, `PH_ML_API_KEY` | – | ML-Dienst |
| `PH_OAUTH_ISSUER_URL`, `PH_OAUTH_CLIENT_ID`, `PH_OAUTH_CLIENT_SECRET` | – | OIDC-Anbieter |
| `PH_BACKUP_CRON`, `PH_BACKUP_KEEP` | `0 0 2 * * *`, `14` | DB-Backup (Cron mit Sekundenfeld) |
| `PH_REVERSE_GEOCODING` | `true` | GeoNames-Daten laden (~10 MB) |
| `PH_WEB_DIR` | `/app/web` | Gebaute Web-App |
| `PUID` / `PGID` | `10001` | Benutzer, unter dem Server und ML-Dienst laufen (Unraid: 99/100) |

Alle Laufzeit-Einstellungen (ML-Schwellen, OAuth, Backup, Video, Papierkorb)
lassen sich zusätzlich unter `/api/admin/config` bzw. in der Admin-Oberfläche ändern.

## Entwicklung

Rust wird über Docker gebaut, eine lokale Toolchain ist nicht nötig:

```bash
scripts/cargo.sh test --workspace                   # Unit-Tests
scripts/cargo.sh build --release -p pixelhort-server
PH_CRATE_DIR=crates/ml scripts/cargo.sh build --release
cd web && npm install && npm run dev                 # Web-App mit Proxy auf :2342
```

End-to-End-Test (Postgres, ML und Server laufen bereits, Testmedien mit
`tests/prepare_media.sh` erzeugt):

```bash
python tests/e2e.py --media ./testdata/media --ext-path /mnt/ext
```

Laufzeitwerkzeuge des Servers: `ffmpeg`/`ffprobe`, `exiftool`,
`heif-convert` (libheif) und `pg_dump` 16. Das Docker-Image bringt alle mit.
Fehlen sie, startet der Server trotzdem und meldet die eingeschränkten Funktionen
im Log.

## Lizenzen der Modelle

CLIP (OpenAI, MIT) und PP-OCR (Apache-2.0) sind frei nutzbar. Das Gesichtsmodell
**InsightFace buffalo_l ist nur für nicht-kommerzielle Nutzung lizenziert**.
