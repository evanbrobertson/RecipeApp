#!/bin/sh
# Starts Crumb as the unprivileged `crumb` user.
#
# Volumes (Railway's included) are mounted owned by root, and volumes created before the image
# stopped running as root hold root-owned files. So the container starts as root just long enough
# to hand the data directory to `crumb` (once: it skips a directory that is already owned by it),
# then drops privileges for good. Set CRUMB_RUN_AS_ROOT=1 to skip all of it and stay root.
set -eu

if [ "$(id -u)" != "0" ] || [ "${CRUMB_RUN_AS_ROOT:-}" = "1" ]; then
  exec "$@"
fi

if [ -n "${DATABASE_PATH:-}" ]; then
  data_dir=$(dirname "$DATABASE_PATH")
elif [ -n "${RAILWAY_VOLUME_MOUNT_PATH:-}" ]; then
  data_dir=$RAILWAY_VOLUME_MOUNT_PATH
else
  data_dir=/app/.data
fi

mkdir -p "$data_dir" 2>/dev/null || true
owner=$(stat -c %u "$data_dir" 2>/dev/null || echo unknown)
want=$(id -u crumb)
# A top directory owned by crumb can still hold root-owned files from an earlier run as root
# (the database, its -wal and -shm), so look one level down as well.
stale=$(find "$data_dir" -maxdepth 2 ! -user crumb -print -quit 2>/dev/null || true)
if [ "$owner" != "$want" ] || [ -n "$stale" ]; then
  echo "[entrypoint] giving $data_dir to the crumb user (one-time)"
  if ! chown -R crumb:crumb "$data_dir"; then
    echo "[entrypoint] WARNING: couldn't chown $data_dir; running as root so the data stays writable." >&2
    echo "[entrypoint] Fix the volume's ownership to run unprivileged." >&2
    exec "$@"
  fi
fi

exec setpriv --reuid=crumb --regid=crumb --init-groups "$@"
