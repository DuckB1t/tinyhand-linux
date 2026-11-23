# Install and troubleshoot

## Standard install

```bash
cd /path/to/tinyhand-linux
./install.sh
```

This creates:

- `~/.local/share/tinyhand-linux/venv/` with the Python environment.
- `~/.local/share/tinyhand-linux/venv/bin/tinyhand` launcher.
- `~/.local/share/tinyhand-linux/venv/bin/tinyhandctl` control client.
- Desktop entries in `~/.local/share/applications/`.
- An icon in `~/.local/share/icons/hicolor/512x512/apps/`.
- `~/.config/tinyhand-linux/config.json` on first run.

It does **not** edit Hyprland/Tacit config or enable autostart.

## Add optional shortcuts

Plain Hyprland: copy lines from `hyprland.conf.snippet`.
Tacit/Lua: copy lines from `tacit.lua.snippet` into
`~/.config/hypr/custom/keybinds.lua`; optional autostart goes in `execs.lua`.
Then `hyprctl reload`.

## Session requirements

```bash
echo "$XDG_CURRENT_DESKTOP"   # Hyprland / Tacit
echo "$WAYLAND_DISPLAY"       # e.g. wayland-1
id -nG | tr ' ' '\n' | grep input
ls -l /dev/input/event*
```

TinyHand needs a compositor supporting `zwlr_layer_shell_v1`. X11 sessions are
not supported. Mouse buttons are read from evdev; if your user cannot read
`/dev/input/event*`, add yourself to `input` and re-login, or install an
udev rule.

## Verify

```bash
~/.local/share/tinyhand-linux/venv/bin/tinyhandctl status
hyprctl layers | grep tinyhand-linux
grim -o $(hyprctl monitors -j | jq -r '.[0].name') /tmp/tinyhand-check.png
```

Expected layer geometry is the full output (e.g. `0 0 1920 1080`).

## Uninstall

```bash
rm -rf ~/.local/share/tinyhand-linux
rm -f ~/.local/share/applications/tinyhand-linux*.desktop
rm -f ~/.local/share/icons/hicolor/512x512/apps/tinyhand-linux.png
rm -f ~/.config/tinyhand-linux/config.json
```

Also remove any keybinds/autostart lines you copied into your WM config.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| `Missing wl_compositor …` | Not a wlroots Wayland session, or layer shell unsupported |
| Drawing does nothing | Check `/dev/input` permissions and that the pointer is active |
| No tap sound | Install PipeWire/PulseAudio client tools so `paplay` exists |
| Duplicate overlay | Only one instance can own the lock/socket; quit extras |
| Clicks also affect apps | Expected on Wayland for drawing; UI panels use the input region to capture clicks |
| Cursor still visible | Expected; Wayland clients cannot hide the global cursor |
