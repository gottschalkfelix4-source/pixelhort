# Pixelhort – Android-App

Native Android-Client für den Pixelhort-Server (Kotlin, Jetpack Compose, Material 3).
Die REST-Schnittstelle ist in [`docs/API.md`](../../docs/API.md) beschrieben.

## Bauen

Voraussetzungen: JDK 17+ (getestet mit JDK 21), Android SDK mit Platform 36 und Build-Tools 36.0.0.
`local.properties` mit `sdk.dir=...` anlegen (wird nicht eingecheckt), dann:

```sh
cd mobile/android
./gradlew assembleDebug          # APK: app/build/outputs/apk/debug/app-debug.apk
./gradlew testDebugUnitTest      # Unit-Tests
```

Toolchain: Gradle 9.3.1 (Wrapper), AGP 8.13.1, Kotlin 2.2.21, KSP 2.2.21-2.0.5,
compileSdk 36, targetSdk 35, minSdk 26, applicationId `app.pixelhort`.

## Funktionen

* **Onboarding** – Server-URL (Prüfung über `/api/server/ping`), Login mit E-Mail/Passwort
  (`deviceType: "android"`), OAuth/SSO über Custom Tabs mit Redirect `app.pixelhort:/oauth-callback`.
* **Automatische Sicherung** – Geräte-Alben (MediaStore-Buckets) auswählen; Optionen nur WLAN,
  nur beim Laden, Videos, Hintergrund an/aus; „Jetzt sichern“; Statusseite (gesichert/ausstehend/Fehler).
* **Zeitleiste** – Monats-Buckets mit Lazy-Loading pro Monat, Gruppierung nach Tag, Pull-to-Refresh,
  Erinnerungen-Karussell, Mehrfachauswahl (Favorit, Archiv, Löschen, Album, Link teilen, gesperrter Ordner).
* **Viewer** – Pager mit Pinch-/Doppeltipp-Zoom, Videowiedergabe (ExoPlayer), Live-/Bewegungsfotos
  (gedrückt halten), Info-Sheet (EXIF, Ort, Personen), Favorit/Löschen/Teilen/Download, **Google Cast**.
* Alben (inkl. geteilter Alben, Benutzer hinzufügen), Favoriten, Archiv, Papierkorb, Suche
  (intelligent/Dateiname/OCR-Text + Filter), Entdecken (Personen, Orte, Dinge), Personen umbenennen,
  Karte (osmdroid/OpenStreetMap), geteilte Links, Partner-Freigabe, gesperrter Ordner (PIN, optional
  Biometrie vorab), Einstellungen (Server, Konto, Sicherung, Abmelden).

## Architektur

```
app/src/main/java/app/pixelhort/
  PixelhortApp.kt        Application + AppContainer (manuelle DI), Coil-ImageLoader mit Auth-OkHttp
  MainActivity.kt        einzige Activity (FragmentActivity für BiometricPrompt/Cast), OAuth-Deep-Link
  data/api/              Retrofit-Interface + kotlinx.serialization-DTOs, ApiClient (Bearer-Token nur für den eigenen Server)
  data/settings/         DataStore (Server, Optionen, Geräte-ID) + EncryptedSharedPreferences (Token)
  data/db/               Room: Sicherungsstatus pro MediaStore-Element
  backup/                MediaStoreScanner, SHA-1-Hashing, BackupEngine, WorkManager-Worker/Scheduler
  cast/                  CastOptionsProvider (Default Media Receiver) + Laden über /cast-url
  ui/                    Compose-Screens (timeline, viewer, albums, search, people, map, sharing, locked, backup, settings)
```

### Ablauf der Sicherung

1. `BackupEngine.syncIndex` gleicht die gewählten MediaStore-Buckets mit der Room-Tabelle ab
   (neue Elemente → `PENDING`, gelöschte werden entfernt).
2. Ausstehende Elemente werden in Blöcken zu 50 per SHA-1 gehasht (gestreamt, Original via
   `MediaStore.setRequireOriginal`, damit GPS-EXIF erhalten bleibt) und per
   `POST /api/assets/bulk-upload-check` geprüft.
3. Abgelehnte (Duplikate) werden als `DUPLICATE` markiert, akzeptierte per Multipart
   `POST /api/assets` gestreamt hochgeladen (`deviceAssetId` = MediaStore-ID, `deviceId` = zufällige
   UUID pro Installation, `x-checksum`-Header). Fehler zählen Versuche hoch (max. 5).
4. Auslöser: periodischer Worker (1 h, Constraints WLAN/Laden), Content-URI-Trigger auf
   MediaStore-Bilder/-Videos (one-shot, wird nach jedem Auslösen neu angemeldet) und „Jetzt sichern“
   (expedited). Läufe mit mehr als 10 Elementen laufen als Foreground-Service (`dataSync`) mit
   Fortschrittsbenachrichtigung. Netz-/Serverfehler → `Result.retry()` mit exponentiellem Backoff.

Bewegungsfotos werden als Original-JPEG hochgeladen; der Server extrahiert das Video.

## Hinweise

* Klartext-HTTP und Benutzer-CAs sind erlaubt (`network_security_config.xml`), da selbst gehostete
  Server oft per LAN-IP oder mit eigener CA erreichbar sind.
* Die Oberfläche ist deutsch (`res/values/strings.xml`).
