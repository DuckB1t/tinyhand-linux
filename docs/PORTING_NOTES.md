# Porting notes and limitations

## Why a reimplementation

Upstream TinyHand is Swift + AppKit and depends on macOS-only APIs: CGEventTap,
CGS private cursor APIs, Carbon hotkeys, NSAccessibility, Sparkle, and a
menu-bar status item. The binary cannot run on Linux, so this port preserves
behaviour and visuals while replacing platform plumbing.

## Layer-shell design

One fullscreen `zwlr_layer_shell_v1` surface is created on the `overlay` layer
with all four anchors and an exclusive zone of `-1`. It uses a double-buffered
`wl_shm` pool and Cairo for drawing. Its input region is empty while the hand
is just pointing, so clicks reach apps underneath. When the toolbar, settings,
or welcome card is visible, the input region is changed to those rectangles so
UI clicks are captured.

## Input model

- Pointer position: Hyprland `.socket2.sock` `pointermove` events with
  `hyprctl -j cursorpos` fallback.
- Buttons and keyboard: evdev. Multiple mice are supported with a short
  debounce window.
- Global hotkeys cannot be registered by an ordinary Wayland client; the
  optional keybind snippets call `tinyhandctl` via your WM.

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

- `py_compile` / imports under the installed venv.
- Live Hyprland layer creation at 1920×1080.
- Control socket commands: `status`, `on`, `off`, `toggle`, `clear`, `demo`,
  `style`, `sound`, `settings`, `section`, `toolbar`, `quit`.
- Screenshot capture with `grim` for welcome, toolbar, and every settings tab.
- Pixel-difference checks confirming overlay and ink changes.

## Known rough edges

- Multi-monitor setups use the composited global cursor space; the overlay is
  one fullscreen layer surface on the primary output.
- The settings picker has a scroll region but no drag-to-reorder animation.
- Custom PNG import is documented but not implemented in the settings UI.
- Wayland keyboard layout is assumed US for the built-in text fields.
