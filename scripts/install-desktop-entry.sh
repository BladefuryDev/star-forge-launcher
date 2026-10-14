#!/usr/bin/env bash
# Registers a locally built launcher with the Linux desktop so Wayland taskbars
# show its icon. They match a window's app id to a .desktop file and ignore the
# icon the window sets itself; packaged builds (deb/rpm/AppImage) ship this.
#   scripts/install-desktop-entry.sh [path/to/star-forge]
# The binary defaults to $STAR_FORGE_BIN (see .env.example), then target/debug.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
[ -f .env ] && { set -a; . ./.env; set +a; }

BIN=$(realpath "${1:-${STAR_FORGE_BIN:-target/debug/star-forge}}")
[ -x "$BIN" ] || { echo "no launcher binary at $BIN; build it first" >&2; exit 1; }
DATA=${XDG_DATA_HOME:-$HOME/.local/share}
ICONS=gui/src-tauri/icons

install -Dm644 "$ICONS/32x32.png" "$DATA/icons/hicolor/32x32/apps/star-forge.png"
install -Dm644 "$ICONS/64x64.png" "$DATA/icons/hicolor/64x64/apps/star-forge.png"
install -Dm644 "$ICONS/128x128.png" "$DATA/icons/hicolor/128x128/apps/star-forge.png"
install -Dm644 "$ICONS/128x128@2x.png" "$DATA/icons/hicolor/256x256/apps/star-forge.png"
install -Dm644 "$ICONS/icon.png" "$DATA/icons/hicolor/512x512/apps/star-forge.png"

# The file name and StartupWMClass both match the app id (the Tauri identifier).
install -Dm644 /dev/stdin "$DATA/applications/star-forge.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=STAR FORGE Launcher
Comment=Star Conflict profile launcher
Exec="$BIN"
Icon=star-forge
StartupWMClass=star-forge
Categories=Game;
EOF

update-desktop-database "$DATA/applications" 2>/dev/null || true
gtk-update-icon-cache -q "$DATA/icons/hicolor" 2>/dev/null || true
echo "installed $DATA/applications/star-forge.desktop -> $BIN"
