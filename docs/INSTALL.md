# Install and troubleshoot

## Standard install

```bash
cd /path/to/tinyhand-linux
./install.sh
```

The installer builds the native Rust release binaries and creates:

- `~/.local/bin/tinyhand` — presentation pointer daemon.
- `~/.local/bin/tinyhandctl` — control client.
- `~/.local/share/tinyhand-linux/assets/` — original hands, sounds, and icon.
- Desktop entries in `~/.local/share/applications/`.
- An icon in `~/.local/share/icons/hicolor/512x512/apps/`.
- `~/.config/tinyhand-linux/config.json` on first run.

It does **not** edit Hyprland/Tacit config, enable autostart, or install any
keybinds. Nothing under `~/.config/hypr/**` is touched.

Set a different install prefix with `XDG_BIN_HOME` and/or `XDG_DATA_HOME` if
you want the binaries or assets elsewhere:

```bash
XDG_BIN_HOME="$HOME/.local/bin" XDG_DATA_HOME="$HOME/.local/share" ./install.sh
```

## Add optional shortcuts

Plain Hyprland: copy lines from `hyprland.conf.snippet`.
Tacit/Lua: copy lines from `tacit.lua.snippet` into
`~/.config/hypr/custom/keybinds.lua`; optional autostart goes in `execs.lua`.
Then `hyprctl reload`.

TinyHand already reads `Ctrl+Alt+H/X/E/M/S` globally through evdev when it can
read `/dev/input/event*`. The snippets are only a fallback.

## Session requirements

- Rust and Cargo are required to build from source.
- A compositor supporting `zwlr_layer_shell_v1`: Hyprland, sway, river, labwc,
  wayfire, etc. X11 is not supported.
- Sound playback uses `paplay` from PipeWire/PulseAudio client tools. Sounds
  are skipped quietly when `paplay` is unavailable.
- Mouse buttons and hotkeys use evdev. If your user cannot read
  `/dev/input/event*`, add yourself to the `input` group and re-login, or
  install a udev rule.

```bash
echo "$XDG_CURRENT_DESKTOP"   # Hyprland / Tacit
echo "$WAYLAND_DISPLAY"       # e.g. wayland-1
id -nG | tr ' ' '\n' | grep input
ls -l /dev/input/event*
```

## Verify

```bash
~/.local/bin/tinyhandctl status
hyprctl layers | grep tinyhand-linux
grim -o $(hyprctl monitors -j | jq -r '.[0].name') /tmp/tinyhand-check.png
```

Expected status uses the original Python-style formatting, for example:

```text
active=True  style=lavender  sound=True  strokes=0  cursor=[0, 0]  settings=False  toolbar=False  reveal=0.0  show_toolbar=True  widgets=0  version=1.1.0
```

Expected layer geometry is the full output, for example `0 0 1920 1080`.

## Uninstall

```bash
rm -f ~/.local/bin/tinyhand ~/.local/bin/tinyhandctl
rm -rf ~/.local/share/tinyhand-linux
rm -f ~/.local/share/applications/tinyhand-linux*.desktop
rm -f ~/.local/share/icons/hicolor/512x512/apps/tinyhand-linux.png
rm -f ~/.config/tinyhand-linux/config.json
```

Also remove any keybinds/autostart lines you copied into your WM config.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| `cargo is required` | Install Rust via rustup or your distribution package manager |
| Missing `wl_compositor` / layer shell | Not a wlroots Wayland session, or layer shell unsupported |
| Drawing does nothing | Check `/dev/input` permissions and that the pointer is active |
| No tap sound | Install PipeWire/PulseAudio client tools so `paplay` exists |
| Duplicate overlay | Only one instance can own the lock/socket; quit extras |
| Clicks also affect apps | Expected on Wayland for drawing; UI panels use the input region to capture clicks |
| Cursor still visible | Expected; Wayland clients cannot hide the global cursor |
