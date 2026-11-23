# UI reference (pixel-level)

This is the specification extracted from upstream 1.1.0 and used by
`src/tinyhand_linux/ui.py`. Coordinates are in logical points at 1× scale.

## Overlay and panels

| Element | Upstream source | Size / style |
| --- | --- | --- |
| Overlay window | `OverlayController.swift:57-71` | Full main screen, borderless, transparent, `ignoresMouseEvents`, level `screenSaver+2`, joins all spaces |
| Main toolbar | `Toolbar.swift:36-62` | 300×56 initial, content-sized; dark HUD glass; 10 pt side / 8 pt vertical padding; radius 16 |
| Ink tray | `Toolbar.swift:64-76` | 200×40 initial, radius 14, 5 pt vertical / 6 pt horizontal padding, swatches 30 pt, spacing 2 |
| Hint panel | `Toolbar.swift:280-292` | 300×36 initial, radius 12, 9 pt vertical / 16 pt horizontal padding, spacing 14, click-through |
| Settings | `SettingsWindow.swift:5-85` | 780×560, hidden title, transparent titlebar, 210 pt sidebar, 22 pt page spacing |
| Welcome card | `WelcomeWindow.swift:75-90` | 340×300, borderless, radius 20, draggable by background |

## Toolbar

- Left→right: pinned style buttons, 1×24 divider at 18% white, ink dot,
  speaker, X.
- Buttons are 40×40, corner radius 10, spacing 6, tooltips set.
- Selected style background: white alpha 0.18.
- Panel origin: horizontal centre, 14 pt above visible bottom.
- Reveal animation 0.15 s; hide 0.35 s. Reveal zone is toolbar frame extended
  80 pt left/right, 90 pt up, 10 pt down; also revealed for 3 s after
  activation and 10 s on first run.
- Ink tray opens 8 pt above the main panel, animates 0.12 s.
- Hint opens 12 pt above the panel, animates 0.2 s in / 0.3 s out.

### Generated artwork

- `inkDot`: 64×64, radius 13, shadow ellipse alpha 0.35 offset +2.5,
  specular highlight alpha 0.35.
- `rainbowDot`: six 60° wedges starting at 90° using system
  red/orange/yellow/green/blue/purple.
- Laser icon: radial red glow plus red disc radius 7 and pale core radius 3.

## Settings

- Sidebar rows: 190×38 at x+10, starting y+52 with 44 pt pitch.
- Icon tile 24×24 radius 6 with vertical gradient; label 13 pt.
- Page title 22 pt bold; page content padding 28 pt horizontal / 48 pt top.
- Cards use radius 12 and 4.5% primary fill; rows are 44 pt min height with
  14 pt horizontal / 10 pt vertical padding and 1 pt dividers.
- Pill buttons: 12 pt medium text, 12×6 padding, capsule, 9% primary fill.
- Shortcut fields: 132×30, radius 7, 5% fill, 18% border; keycaps 20 pt high,
  22 pt minimum width, radius 5, 10% fill / 18% border.

### Sections

1. **General** — permission, two shortcut fields, hold-to-use key, while
   holding, double-tap Shift, startup/updates.
2. **Pointers** — 8 toolbar slots with 58 pt tall rounded backgrounds,
   add/empty slots, style picker (400×340, adaptive 64–72 pt grid), toolbar
   switches, custom hands, make-hand subpage.
3. **Appearance** — 180 pt size slider (0.08–0.20), shadow switch.
4. **Drawing** — 9 ink swatches (20 pt circles), fade picker.
5. **Sound** — adaptive tile grid (104–140 pt), Off + 15 sounds, laser switch.
6. **About** — 72 pt icon, links, reset-all.

## Welcome

- Icon 76 pt, title 19 pt semibold, subtitle 13 pt secondary, button 180 pt
  wide, tip 11 pt secondary. Custom spacings: 16 after icon, 22 after
  subtitle, 12 after button.

## Colours

- Purple `#8754F2`, red `#FF2920`, orange `#FF8A00`, yellow `#FFCC00`,
  green `#30C25A`, blue `#0A84FF`, white `#FFFFFF`, black `#1D1D21`.
- Style-specific inks include `#2F86F0`, `#16A87A`, `#EE4F98`, `#F2B705`,
  `#6B7380`, `#9A5E2C`, `#F0561D`, `#1597E5`, `#A33FE6`, `#3E9E2E`,
  `#E0309A`.
- Dark glass tint is white 0.07 alpha 0.62 with 1 pt white 0.12 border.

## Hand assets

- All 36 `hand_*.png` files are 1024×1536 RGBA.
- The Linux renderer uses the original files and the measured geometry in
  `assets/manifest.json`, so style silhouettes and grip points match upstream
  rather than being redrawn approximations.
