#!/usr/bin/env bash
# Build and install the native Rust TinyHand Linux binaries.
# This script deliberately does NOT edit Hyprland/Tacit config or enable autostart.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_HOME="${XDG_BIN_HOME:-$HOME/.local/bin}"
APP_DATA="$DATA_HOME/tinyhand-linux"
ASSETS="$APP_DATA/assets"
APPLICATIONS="$DATA_HOME/applications"
ICONS="$DATA_HOME/icons/hicolor/512x512/apps"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required to build TinyHand Linux" >&2
  exit 1
fi

echo "Building Rust release binaries..."
cargo build --release --manifest-path "$ROOT/rust/Cargo.toml"

mkdir -p "$BIN_HOME" "$APPLICATIONS" "$ICONS" "$ASSETS" "${XDG_CONFIG_HOME:-$HOME/.config}/tinyhand-linux"
install -m 0755 "$ROOT/rust/target/release/tinyhand" "$BIN_HOME/tinyhand"
install -m 0755 "$ROOT/rust/target/release/tinyhandctl" "$BIN_HOME/tinyhandctl"
cp -a "$ROOT/src/tinyhand_linux/assets/." "$ASSETS/"
cp "$ROOT/src/tinyhand_linux/assets/icons/tinyhand-linux.png" "$ICONS/tinyhand-linux.png"

cat > "$APPLICATIONS/tinyhand-linux.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=TinyHand Linux
GenericName=Screen pointer
Comment=A hand-on-a-stick pointer for Wayland screen sharing
Exec=$BIN_HOME/tinyhand
Icon=tinyhand-linux
Terminal=false
Categories=Utility;Accessibility;
EOF

cat > "$APPLICATIONS/tinyhand-linux-url.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=TinyHand URL handler
Exec=$BIN_HOME/tinyhand %u
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

  Run:          $BIN_HOME/tinyhand
  Control:      $BIN_HOME/tinyhandctl status
  Settings:     $BIN_HOME/tinyhandctl settings
  Shortcuts:    see $ROOT/hyprland.conf.snippet (nothing was changed automatically)

Make sure $BIN_HOME is in your PATH.

To uninstall:
  rm -f "$BIN_HOME/tinyhand" "$BIN_HOME/tinyhandctl"
  rm -rf "$APP_DATA"
  rm -f "$APPLICATIONS/tinyhand-linux.desktop" "$APPLICATIONS/tinyhand-linux-url.desktop"
  rm -f "$ICONS/tinyhand-linux.png"
EOF
