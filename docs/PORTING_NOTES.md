# Porting notes and limitations

## Why a reimplementation

Upstream TinyHand is Swift + AppKit and depends on macOS-only APIs: CGEventTap,
CGS private cursor APIs, Carbon hotkeys, NSAccessibility, Sparkle, and a
menu-bar status item. The binary cannot run on Linux, so this port preserves
behaviour and visuals while replacing platform plumbing.

## Layer-shell design

One fullscreen `zwlr_layer_shell_v1` surface is created on the `overlay` layer
with all four anchors and an exclusive zone of `-1`. It uses two `wl_shm`
buffers in private pools and Cairo for drawing. Its input region is empty while the hand
is just pointing, so clicks reach apps underneath. When the toolbar, settings,
or welcome card is visible, the input region is changed to those rectangles so
UI clicks are captured.

## Input model

- Pointer position: Hyprland `.socket2.sock` `pointermove` events with
  `hyprctl -j cursorpos` fallback.
- Buttons and keyboard: evdev. Multiple mice are supported with a short
  debounce window.
- Global hotkeys are read via evdev when `/dev/input` is readable, so the
  built-in `Ctrl+Alt+H/X/E/M/S` shortcuts work without WM configuration. If
  evdev access is unavailable, the optional keybind snippets call
  `tinyhandctl` through the WM instead.

## Deliberate differences

| Upstream | Linux port |
| --- | --- |
| Swallows normal clicks while active | Cannot; clicks still reach apps during drawing |
| Hides the system cursor | Cannot; real cursor stays visible |
| Menu-bar status item and NSMenu | No status item on Hyprland; use CLI/settings/URL commands |
| Sparkle updates | No updater; install from source/package manager |
| Custom hand generator uses ChatGPT + macOS APIs | UI scaffold only; documented manual workflow |
| Native SwiftUI settings | Cairo-drawn equivalent with same dimensions and labels |

## Verification performed

- `cargo build --release` and `cargo test` for the Rust implementation.
- The 27-test offline Python oracle suite still passes; it is kept only as a
  behavioural reference, not as a runtime dependency.
- Live Rust daemon on Hyprland/Wayland with a full 1920×1080 layer surface.
- Control socket and URL commands: `status`, `on`, `off`, `toggle`, `clear`,
  `demo`, `style`, `sound`, `settings`, `section`, `toolbar`, `welcome`, `quit`,
  plus `tinyhand://style/coral?x=1`-style URLs.
- Python-compatible control output, including `active=True style=lavender
  sound=True` rather than JSON booleans or quoted strings.
- Screenshot capture with `grim` and visual comparison against the Python
  reference for the toolbar/hand overlay.
- No changes made to `~/.config/hypr/**`; no autostart or keybinds installed.

## Known rough edges

- Multi-monitor setups use the composited global cursor space; the overlay is
  one fullscreen layer surface on the primary output.
- The settings picker has a scroll region but no drag-to-reorder animation.
- Custom PNG import is documented but not implemented in the settings UI.
- Wayland keyboard layout is assumed US for the built-in text fields.
