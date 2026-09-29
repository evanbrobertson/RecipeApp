#!/usr/bin/env bash
# Install Crumb's Linux desktop app for the current user, from an unpacked tarball.
#
#   ./install.sh              install (or update) in place
#   ./install.sh --uninstall  remove everything this script installed
#
# Everything lands under the user's home, so nothing here needs root.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
bin_dir="$HOME/.local/bin"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
apps_dir="$data_home/applications"
icon_dir="$data_home/icons/hicolor/512x512/apps"

binary="$bin_dir/crumb-desktop"
desktop="$apps_dir/crumb-desktop.desktop"
icon="$icon_dir/crumb-desktop.png"
# Written by update-desktop-database alongside the entries
cache="$apps_dir/mimeinfo.cache"

refresh_desktop_database() {
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$apps_dir" >/dev/null 2>&1 || true
  fi
}

if [[ ${1:-} == --uninstall ]]; then
  rm -f "$binary" "$desktop" "$icon"
  refresh_desktop_database
  # update-desktop-database leaves the cache behind; drop it when no other app is listed
  if [[ -f $cache ]] && ! grep -q '=' "$cache"; then
    rm -f "$cache"
  fi
  echo "Removed crumb-desktop from $bin_dir, $apps_dir and $icon_dir."
  exit 0
fi

mkdir -p "$bin_dir" "$apps_dir" "$icon_dir"
install -m755 "$here/crumb-desktop" "$binary"
install -m644 "$here/crumb-desktop.png" "$icon"
# Point the entry at the installed files, not the names a system package would use
sed -e "s|^Exec=.*|Exec=$binary|" -e "s|^Icon=.*|Icon=$icon|" \
  "$here/crumb-desktop.desktop" > "$desktop"
refresh_desktop_database

echo "Installed crumb-desktop to $binary"
echo "Desktop entry: $desktop"
echo "Icon: $icon"
echo "Make sure $bin_dir is on your PATH."
echo
echo "To uninstall: $0 --uninstall"
