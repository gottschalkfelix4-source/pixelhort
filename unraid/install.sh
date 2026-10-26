#!/bin/sh
# Installs the Pixelhort Unraid templates and icons from the server image, so it
# works without access to the (private) GitHub repository:
#
#   docker run --rm --entrypoint /usr/share/pixelhort/unraid/install.sh \
#     -v /boot/config/plugins/dockerMan:/dockerman \
#     ghcr.io/gottschalkfelix4-source/pixelhort-server:latest
#
# Existing templates are kept (they hold the settings of installed containers),
# unless they are empty/broken or --force is given.
set -e

SRC="$(dirname "$0")"
DST="${DOCKERMAN:-/dockerman}"
FORCE=0
[ "$1" = "--force" ] && FORCE=1

if [ ! -d "$DST" ]; then
    echo "Fehler: $DST fehlt – mit -v /boot/config/plugins/dockerMan:$DST starten." >&2
    exit 1
fi
mkdir -p "$DST/templates-user" "$DST/images"

for t in pixelhort-db pixelhort-ml pixelhort; do
    out="$DST/templates-user/my-$t.xml"
    # Keep a template only when it actually contains a container definition.
    if [ "$FORCE" = 0 ] && [ -s "$out" ] && grep -q "<Container" "$out"; then
        echo "übersprungen: my-$t.xml existiert bereits (mit --force überschreiben)"
    else
        cp "$SRC/$t.xml" "$out"
        echo "installiert:  my-$t.xml"
    fi
    # dockerMan uses <name>-icon.png from here before trying the Icon URL.
    cp "$SRC/pixelhort.png" "$DST/images/$t-icon.png"
done

echo "Fertig. Jetzt unter Docker → Container hinzufügen nacheinander pixelhort-db, pixelhort-ml und pixelhort wählen."
