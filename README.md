# TinyHand Linux

A native Linux/Wayland port of [TinyHand](https://github.com/nilbuild/tinyhand).
TinyHand is the macOS app that replaces your cursor with a toy hand on a stick
while you present or screen-share, and lets you draw on the screen.

The upstream app is Swift/AppKit and macOS 13+. This repository is a clean
Python reimplementation for wlroots compositors, built from a source-level and
pixel-level study of upstream 1.1.0. It was developed and verified on the
maintainer's **Hyprland 0.57.2 / Tacit / Wayland** session (1920×1080, `eDP-1`).

## What works

- The original **36 hand PNGs** and all **15 tap sounds** are bundled and used
  directly from the upstream repository.
- A fullscreen, always-on-top `zwlr_layer_shell_v1` overlay that follows the
  Hyprland pointer.
- Accurate hand pose rendering: per-asset alpha measurements, grip lean, stick
  length, press animation, shadow, and a laser-dot style.
- Freehand ink with smoothing, arrowheads (hold Ctrl while drawing), colour
  choices, and timed fading.
- Toolbar, ink tray, hint panel, six-section settings window, and welcome card,
  all painted with Cairo to match the upstream AppKit layout.
- 37 pointer styles (36 hands + laser), pinned toolbar reordering placeholder,
  preference persistence, `state.json`, and `tinyhand://` URL commands.
- CLI control via a Unix socket, plus optional keybind snippets for plain
  Hyprland and Tacit/Lua. **No config is edited by the installer.**

## Install

```bash
git clone <this-repo> tinyhand-linux
cd tinyhand-linux
./install.sh
```

The installer creates an isolated venv under
`~/.local/share/tinyhand-linux/venv`, installs desktop entries, and registers
`tinyhand://` URLs. It does **not** touch your Hyprland config, add autostart,
or bind keys. Add shortcuts yourself from `hyprland.conf.snippet` or
`tacit.lua.snippet`.

Requirements:

- Wayland compositor with `zwlr_layer_shell_v1` (Hyprland, sway, river,
  labwc, wayfire, …).
- Python 3.10+.
- Read access to `/dev/input/event*` for mouse buttons (user in `input` group).
- `paplay` (PulseAudio/PipeWire) for tap sounds; silently skipped if absent.

## Run

```bash
~/.local/share/tinyhand-linux/venv/bin/tinyhand
```

Useful commands (also available as `tinyhandctl` in the venv):

```bash
tinyhandctl status
tinyhandctl on | off | toggle
tinyhandctl clear
tinyhandctl demo
tinyhandctl style laser          # or: style next, style lavender, …
tinyhandctl sound on | off | toggle
tinyhandctl settings             # open settings window on the overlay
tinyhandctl section pointers     # open a specific settings tab
tinyhandctl toolbar              # temporarily reveal the toolbar
tinyhandctl quit
tinyhand://toggle                # URL handler, same command set
```

## Controls

| Gesture | Action |
| --- | --- |
| Move mouse | Hand follows (real cursor is not hidden on Wayland) |
| Left-click | Tap ripple + sound |
| Left-drag | Draw a line |
| Ctrl + left-drag | Draw an arrowhead |
| Right-click | Cycle pointer style |
| Hold Option (or configured interact key) | Real interaction with the app underneath |
| Double-tap Shift | Toggle click-through |
| Esc | Exit / turn pointer off |
| `Ctrl+Alt+H` * | Toggle pointer (if you add the keybind) |
| `Ctrl+Alt+X` * | Clear drawing |
| `Ctrl+Alt+E` * | Cycle style |
| `Ctrl+Alt+M` * | Toggle sound |
| `Ctrl+Alt+S` * | Open settings |

\* Optional; see the snippet files.

## Documentation

- [`docs/REVERSE_ENGINEERING.md`](docs/REVERSE_ENGINEERING.md) — how upstream works, source map, constants, and REA findings.
- [`docs/UI_REFERENCE.md`](docs/UI_REFERENCE.md) — pixel-level UI/asset specification used by the port.
- [`docs/INSTALL.md`](docs/INSTALL.md) — detailed install, uninstall, and troubleshooting.
- [`docs/PORTING_NOTES.md`](docs/PORTING_NOTES.md) — macOS→Wayland substitutions and known limitations.
- [`docs/asset_reference.json`](docs/asset_reference.json) — measured hand asset geometry.

## Development

```bash
make deps        # create .venv and install the package in editable mode
make test        # 14 offline unit tests (prefs, assets, Cairo drawing, UI hit-testing)
make run         # start from the source tree
```

`make build` produces wheel/sdist artifacts. Nothing in the Makefile or
installer writes to `~/.config/hypr/**`.

## Repository layout

```
src/tinyhand_linux/
  app.py          application state machine, event loop, commands
  ui.py           toolbar / settings / welcome panels and hit-testing
  draw.py         Cairo primitives, hand renderer, ink renderer
  wayland.py      pywayland layer-shell surface and SHM buffers
  input.py        evdev mouse/keyboard + Hyprland IPC
  prefs.py        preferences, 37 styles, ink and sound catalogues
  paths.py        XDG paths
  assets/         upstream hands, sounds, icon, geometry manifest
  protocol/       generated pywayland bindings
tests/            offline checks
```

## Attribution

Upstream TinyHand © 2026 Kamran Ahmed, MIT licensed. This port keeps the
upstream licence and reuses the original art and audio assets. Reverse
engineering was performed from the public source at commit `da98f98` (v1.1.0).
