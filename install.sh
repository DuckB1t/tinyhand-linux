#!/usr/bin/env bash
# Install TinyHand Linux into an isolated user venv.
# This script deliberately does NOT edit Hyprland/Tacit config or enable autostart.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_DATA="$DATA_HOME/tinyhand-linux"
VENV="$APP_DATA/venv"
APPLICATIONS="$DATA_HOME/applications"
ICONS="$DATA_HOME/icons/hicolor/512x512/apps"

if [ ! -x "$VENV/bin/python" ]; then
  python3 -m venv "$VENV"
fi
"$VENV/bin/pip" install -q --upgrade pip
"$VENV/bin/pip" install -q "$ROOT"

mkdir -p "$APPLICATIONS" "$ICONS" "${XDG_CONFIG_HOME:-$HOME/.config}/tinyhand-linux"
cp "$ROOT/src/tinyhand_linux/assets/icons/tinyhand-linux.png" "$ICONS/tinyhand-linux.png"

cat > "$APPLICATIONS/tinyhand-linux.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=TinyHand Linux
GenericName=Screen pointer
Comment=A hand-on-a-stick pointer for Wayland screen sharing
Exec=$VENV/bin/tinyhand
Icon=tinyhand-linux
Terminal=false
Categories=Utility;Accessibility;
EOF

cat > "$APPLICATIONS/tinyhand-linux-url.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=TinyHand URL handler
Exec=$VENV/bin/tinyhand %u
Icon=tinyhand-linux
Terminal=false
NoDisplay=true
MimeType=x-scheme-handler/tinyhand;
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APPLICATIONS" >/dev/null 2>&1 || true
fi
if command -v xdg-mime >/dev/null 2>&1; then
  xdg-mime default tinyhand-linux-url.desktop x-scheme-handler/tinyhand >/dev/null 2>&1 || true
fi

cat <<EOF

TinyHand Linux installed.

  Run:          $VENV/bin/tinyhand
  Control:      $VENV/bin/tinyhandctl status
  Settings:     $VENV/bin/tinyhandctl settings
  Shortcuts:    see $ROOT/hyprland.conf.snippet (nothing was changed automatically)

To uninstall: rm -rf "$APP_DATA" "$APPLICATIONS/tinyhand-linux.desktop" "$APPLICATIONS/tinyhand-linux-url.desktop"
EOF
