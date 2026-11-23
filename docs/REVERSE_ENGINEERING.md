# Reverse-engineering notes

Source studied: `https://github.com/nilbuild/tinyhand.git`, commit `da98f98`
(`VERSION` = `1.1.0`). Local clone used during development:
`~/tinyhand-source`.

## Method

1. Cloned the public source repository.
2. Mapped all 22 Swift files under `Sources/TinyHand/`.
3. Tried REA's bundle/asset tooling against a reconstructed `.app` resource
   layout. REA reported that Apple asset-catalog and plist inspection require
   a macOS host (`assetutil`, native macOS plist provider), which is
   unavailable on this Linux host. That limitation is recorded here rather
   than silently ignored.
4. Used Pillow/NumPy to measure every shipped PNG pixel-by-pixel: alpha bounds,
   tip/wrist/rod centres, aspect ratios, and thumbnail crops. Results are in
   `docs/asset_reference.json` and `src/tinyhand_linux/assets/manifest.json`.
5. Read the Swift source directly for exact constants, state transitions,
   labels, sizes, colours, and timings. Two independent passes covered
   behaviour and UI/assets; their findings are summarised below.

## App lifecycle

- `main.swift` runs `.accessory` (no Dock icon), with diagnostic CLI modes
  (`--render-test`, `--settings-test`, `--toolbar-test`, `--welcome-test`,
  `--make-hand`, `--e2e-test`, `--click-test`).
- `AppDelegate` preloads hand thumbnails, installs the editing menu, hides the
  cursor in background apps, starts Sparkle, creates the status item, registers
  hotkeys, shows the welcome card if needed, and writes `state.json`.
- Activation refuses to start without Accessibility trust. On Linux this is
  replaced by a Wayland layer-shell capability check at startup.
- Termination deactivates the overlay and deletes `state.json`.

## Pointer / input pipeline (macOS)

- CGEvent session tap with head insertion and a mask covering left/right/other
  down/up/drag events.
- Normal left down/up events are swallowed; drags are rewritten as mouse-moved
  so apps see motion but not dragging.
- The configured interact key (Option by default) is stripped from events and
  starts "real interaction" mode.
- Double-tap Shift with timing windows 0.3 s / 0.35 s toggles click-through.
- Pass-through rectangles exist for the menu bar, Dock, open menus, and the
  app's own toolbar.
- Cursor hiding uses private CGS APIs plus a reassert state machine
  (`1 s` normal, `0.05 s` settling, `0.03 s` fighting, urgent `0`).

## Pointer rendering

- Fullscreen borderless transparent window, level `screenSaver + 2`, belongs to
  all spaces, input-transparent.
- 120 Hz repaint while moving, 20 Hz after 0.5 s of stillness.
- Grip geometry constants: `gripLeanFrac = 0.30`, `gripDepthFrac = 0.12`,
  `gripFollow = 0.08`, `minRodFrac = 0.05`, `liftFrac = 0.06`.
- Default hand length is `0.13 × screenHeight`; press shrinks it by 6%.
- Laser radius is `max(4, screenH × 0.0038 × handLenFrac / 0.13)`, core 45%,
  glow 2.2–3.2×, colour `#FF2920`.
- Hand images are 1024×1536 RGBA. Alpha scanning derives `handTop`, `tipX`,
  `wrist`, `rodEnd`, `rodX`, and aspect for pose calculations.

## Ink

- Width `max(3, screenH × 0.0055)`; round caps/joins; black shadow alpha 0.18,
  offset `(1, -1.5)`, radius 1.5.
- Points closer than 1.5 pt are discarded; a stroke starts after 4 pt of drag.
- Smoothing uses quadratic curves through midpoints.
- Arrowheads appear while Command is held: size `max(14, width × 3.2)`,
  two lines at `angle + π ± π/7`.
- Fade delay `max(0.5, fadeSeconds)`, duration 0.6 s, ease-in; default 3 s,
  choices 3/6/10/keep.

## Styles and assets

- 36 hand styles: lavender, glove, coral, plush, robot, pink, blue, mint,
  lemon, black, chrome, holo, fuzzy, wood, marble, terrazzo, glass, jelly,
  pastel, pixel, lowpoly, knit, skeleton, astro, space, monster, water, lava,
  grass, cactus, choco, cloud, mech, neon, leaf, cookie; plus laser.
- Pinned toolbar default: lavender, glove, coral, plush, robot, laser (max 8).
- 15 tap sounds: thock, click, tock, clack, typewriter, snap, wood, pop, boop,
  blip, coin, pew, squeak, quack, chirp. Default `thock`; laser sound off.
- Menu bar art is 18×18 template PNG with @2x/@3x variants.

## Automation and persistence

- URL scheme `tinyhand://<command>[/<arg>]`: on, off, toggle, clear,
  `style/<id>`, `sound/[on|off|toggle]`.
- `~/Library/Application Support/TinyHand/state.json` contains `pid`, `active`,
  `style`, `soundOn`, and a `styles` catalogue; deleted at quit.
- All preferences live in `UserDefaults` and post a change notification.
- Log: `~/Library/Logs/TinyHand.log`.

## Exact defaults ported

`pointerSize 0.13`, `showShadow true`, `fadeSeconds 3`, `sound.hands true`,
`tapSound thock`, `sound.laser false`, `interactKeepsHand false`,
`doubleShiftClickThrough true`, `interactKey option`, `inkChoice match`,
`showToolbar true`, `showInkButton true`, toggle `⌃⌥H`, clear `⌃⌥X`.

## Linux substitutions

| macOS mechanism | Linux implementation |
| --- | --- |
| Borderless always-on-top NSWindow | `zwlr_layer_shell_v1` overlay layer |
| CGEventTap + event swallowing | Evdev buttons + Hyprland IPC; no global suppression for ordinary clients |
| CGS cursor hiding | Not available to normal Wayland clients; real cursor remains visible |
| Carbon global hotkeys | Optional WM keybinds calling `tinyhandctl` |
| UserDefaults | `$XDG_CONFIG_HOME/tinyhand-linux/config.json` |
| Application Support `state.json` | `$XDG_RUNTIME_DIR/tinyhand-linux-state.json` |
| Sparkle updater | Intentionally omitted for a source-built Linux package |
| SwiftUI settings | Cairo-drawn panels with equivalent dimensions, labels, and controls |
