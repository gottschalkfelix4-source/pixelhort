#!/bin/sh
# Runs the given command as PUID:PGID (default 10001). When started as root it
# first makes the writable directories (PH_WRITABLE_DIRS) owned by that user, so
# host folders created by Docker/Unraid as root work out of the box.
set -e

PUID="${PUID:-10001}"
PGID="${PGID:-10001}"

if [ "$(id -u)" = "0" ]; then
    for dir in ${PH_WRITABLE_DIRS}; do
        mkdir -p "$dir"
        # Only fix what is not ours yet (cheap on large libraries).
        find "$dir" -maxdepth 3 \( ! -user "$PUID" -o ! -group "$PGID" \) -exec chown "$PUID:$PGID" {} + 2>/dev/null || true
    done
    exec setpriv --reuid="$PUID" --regid="$PGID" --clear-groups --inh-caps=-all "$@"
fi

exec "$@"
