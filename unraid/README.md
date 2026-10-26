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

2. **Templates kopieren** (im Unraid-Terminal):

   ```bash
   cd /boot/config/plugins/dockerMan/templates-user
   for t in pixelhort-db pixelhort-ml pixelhort; do
     wget -O my-$t.xml https://raw.githubusercontent.com/gottschalkfelix4-source/pixelhort/main/unraid/$t.xml
   done
   ```

   Alternativ die drei XML-Dateien per Netzwerkfreigabe nach
   `/boot/config/plugins/dockerMan/templates-user/` kopieren (Dateiname mit `my-` beginnen).

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

Werden die Pakete auf GitHub öffentlich gestellt, entfällt das. Template-URL und
Icon (`raw.githubusercontent.com`) funktionieren ebenfalls nur bei einem öffentlichen
Repository. Bei privatem Repo die XML-Dateien manuell kopieren (Schritt 2).

## ML auf einem anderen Rechner

`pixelhort-ml` dort starten (Port 3003 freigeben, `ML-API-Key` setzen) und im
Container `pixelhort` die *ML-Dienst URL* auf `http://<rechner>:3003` und
denselben Key eintragen. Mit NVIDIA-GPU das Repository auf `…/pixelhort-ml:latest-cuda`
ändern, das Nvidia-Driver-Plugin installieren und `--runtime=nvidia` als Extra
Parameter sowie `NVIDIA_VISIBLE_DEVICES` setzen.

## Rechte

Alle Pixelhort-Container laufen als `PUID`/`PGID` (Standard 99:100 = nobody:users).
Beim Start werden die Ordner `/data` bzw. `/cache` einmalig auf diesen Benutzer
umgestellt, auch wenn Unraid sie als root angelegt hat.
