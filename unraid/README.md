# Pixelhort auf Unraid

Drei Container, alle im eigenen Docker-Netzwerk `pixelhort`:

| Template | Container | Zweck |
|---|---|---|
| [pixelhort-db.xml](pixelhort-db.xml) | `pixelhort-db` | PostgreSQL 16 mit pgvector |
| [pixelhort-ml.xml](pixelhort-ml.xml) | `pixelhort-ml` | Machine Learning (optional, auch auf anderem Rechner) |
| [pixelhort.xml](pixelhort.xml) | `pixelhort` | Server, Web-App, API, Jobs |

## Installation

1. **Netzwerk anlegen** (einmalig, im Unraid-Terminal):

   ```bash
   docker network create pixelhort
   ```

   Unter *Einstellungen → Docker* (erweiterte Ansicht) „Benutzerdefinierte Netzwerke
   beibehalten“ auf *Ja* stellen, damit das Netzwerk in der Auswahl erscheint.

2. **Templates installieren** (im Unraid-Terminal). Die Templates samt Icon stecken im
   Server-Image, daher genügt der Registry-Login (siehe [Images](#images)):

   ```bash
   docker run --rm --entrypoint /usr/share/pixelhort/unraid/install.sh \
     -v /boot/config/plugins/dockerMan:/dockerman \
     ghcr.io/gottschalkfelix4-source/pixelhort-server:latest
   ```

   Das legt `my-pixelhort-db.xml`, `my-pixelhort-ml.xml` und `my-pixelhort.xml` unter
   `/boot/config/plugins/dockerMan/templates-user/` an. Vorhandene Templates mit Inhalt
   bleiben erhalten (sie enthalten die Einstellungen installierter Container); leere
   werden ersetzt. Mit `… install.sh --force` (Argument ans Ende anhängen) alle überschreiben.

   Alternativ die drei XML-Dateien per Netzwerkfreigabe nach
   `/boot/config/plugins/dockerMan/templates-user/` kopieren (Dateiname mit `my-` beginnen).
   **Nicht** per `wget` von `raw.githubusercontent.com` laden: Solange das Repository
   privat ist, liefert GitHub dort 404 und es entstehen leere Templates.

3. Unter *Docker → Container hinzufügen* nacheinander die Templates
   **pixelhort-db**, **pixelhort-ml** und **pixelhort** auswählen:
   - In `pixelhort-db` ein eigenes `POSTGRES_PASSWORD` setzen und dasselbe Passwort
     in der *Datenbank-URL* von `pixelhort` eintragen.
   - Für große Fotobestände den Datenordner von `pixelhort` auf eine eigene Freigabe legen
     (z. B. `/mnt/user/pixelhort`).

4. `http://<unraid-ip>:2342` öffnen, das erste Konto wird Administrator. In der
   Mobile-App dieselbe Adresse eintragen.

## Images

Die Images liegen in der GitHub Container Registry und werden bei jedem Push auf
`main` gebaut ([docker.yml](../.github/workflows/docker.yml)):

- `ghcr.io/gottschalkfelix4-source/pixelhort-server:latest`
- `ghcr.io/gottschalkfelix4-source/pixelhort-ml:latest` (CPU)
- `ghcr.io/gottschalkfelix4-source/pixelhort-ml:latest-cuda` (NVIDIA-GPU)

Solange Repository und Pakete **privat** sind, muss sich Unraid einmalig an der
Registry anmelden (Personal Access Token mit `read:packages`):

```bash
docker login ghcr.io -u gottschalkfelix4-source
```

Werden die Pakete auf GitHub öffentlich gestellt, entfällt das. Die Icon-URL
(`raw.githubusercontent.com`) funktioniert nur bei einem öffentlichen Repository; der
Installer legt das Icon deshalb direkt unter `dockerMan/images/` ab.

## ML auf einem anderen Rechner

`pixelhort-ml` dort starten (Port 3003 freigeben, `ML-API-Key` setzen) und im
Container `pixelhort` die *ML-Dienst URL* auf `http://<rechner>:3003` und
denselben Key eintragen. Mit NVIDIA-GPU das Repository auf `…/pixelhort-ml:latest-cuda`
ändern, das Nvidia-Driver-Plugin installieren und `--runtime=nvidia` als Extra
Parameter sowie `NVIDIA_VISIBLE_DEVICES` setzen.

## Casting auf Fernseher (DLNA)

Smart-TVs werden per SSDP-Multicast gesucht. Aus dem Bridge-Netzwerk `pixelhort`
erreicht dieser Multicast das Heimnetz nicht. Zwei Möglichkeiten:

- Fernseher im Viewer unter *Streamen → Gerät per Adresse hinzufügen* mit seiner
  UPnP-Beschreibungs-URL eintragen, z. B. `http://192.168.1.20:9197/dmr` (Samsung)
  oder `http://192.168.1.21:1255/` (LG). Das Gerät wird gespeichert.
- Oder den Container `pixelhort` im Netzwerkmodus `host` betreiben; dann findet
  „Erneut suchen“ die Geräte automatisch.

In beiden Fällen `PH_PUBLIC_URL` auf die LAN-Adresse des Servers setzen
(z. B. `http://192.168.1.10:2342`), damit der Fernseher die Medien abrufen kann.

## Rechte

Alle Pixelhort-Container laufen als `PUID`/`PGID` (Standard 99:100 = nobody:users).
Beim Start werden die Ordner `/data` bzw. `/cache` einmalig auf diesen Benutzer
umgestellt, auch wenn Unraid sie als root angelegt hat.
