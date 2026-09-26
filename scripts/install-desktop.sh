#!/usr/bin/env bash
# Build Crumb and install its native GTK shell for the current user.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
for program in cargo bun python3; do
  command -v "$program" >/dev/null || { echo "Missing $program" >&2; exit 1; }
done
python3 -c 'import gi; gi.require_version("Gtk", "3.0"); gi.require_version("WebKit2", "4.1"); from gi.repository import Gtk, WebKit2' || {
  echo 'Install python-gobject, gtk3, and webkit2gtk-4.1 first.' >&2
  exit 1
}

(cd "$repo/web" && bun install --frozen-lockfile && bun run build)
(cd "$repo" && cargo build --release)

data="${XDG_DATA_HOME:-$HOME/.local/share}"
root="$data/crumb-desktop"
mkdir -p "$root/bin" "$root/desktop" "$root/web" "$HOME/.local/bin" \
  "$data/applications" "$data/icons/hicolor/512x512/apps"
install -m 755 "$repo/target/release/crumb" "$root/bin/crumb"
install -m 644 "$repo/desktop/omarchy_theme.py" "$root/desktop/omarchy_theme.py"
cp -a "$repo/web/dist/." "$root/web/"
install -m 755 "$repo/desktop/crumb-desktop.py" "$HOME/.local/bin/crumb-desktop"
install -m 644 "$repo/web/public/icon-512.png" "$data/icons/hicolor/512x512/apps/crumb.png"
cat > "$data/applications/crumb-desktop.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Crumb
Comment=Your private recipe box
Exec=$HOME/.local/bin/crumb-desktop
Icon=crumb
Terminal=false
Categories=Utility;
StartupNotify=true
StartupWMClass=crumb-desktop
EOF
echo "Crumb is installed. Open it from the app launcher or run $HOME/.local/bin/crumb-desktop"
