<div align="center">

# TinyHand Linux

### The playful presentation pointer, now native on Wayland.

**A fast, small Rust/Cairo port of [TinyHand](https://github.com/nilbuild/tinyhand) for Hyprland, sway, river, and other wlroots compositors.**

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20Wayland-00D1B2?logo=linux)](https://wayland.freedesktop.org/)
[![WM](https://img.shields.io/badge/WM-Hyprland%20%2F%20wlroots-7B68EE)](https://hyprland.org/)

`Rust` · `Cairo` · `native Wayland` · `evdev` · `Hyprland IPC` · `layer-shell`

</div>

---

## ✨ What it is

TinyHand replaces your cursor with a toy hand on a stick while you present or screen-share. This Linux edition keeps the original's spirit — 36 hand styles, tap sounds, ink, toolbar, settings, and the same measured geometry — but runs on a native Rust stack with no Python, Pillow, NumPy, pip, or venv at runtime.

It was developed and verified on a real **Hyprland / Tacit / Wayland** session at **1920×1080** on `eDP-1`.

> 🛡️ **No surprise system changes.** The installer never edits `~/.config/hypr/**`, never enables autostart, and never binds keys. Optional snippets are provided for you to copy only if you want them.

---

## 🚀 Why the Rust port

| | Feature | What you get |
|---|---|---|
| 🖐️ | **Original art, not approximations** | All 36 upstream `hand_*.png` files and 15 `tap_*.wav` files are bundled and used directly. |
| 🦀 | **Native Rust runtime** | Smaller and faster than the Python implementation, with no interpreter startup or virtualenv. |
| 🎯 | **Pixel-minded rendering** | The same grip lean, stick length, press animation, shadow, and laser-dot behavior as the reference port. |
| 🖍️ | **Screen ink** | Smooth freehand strokes, Ctrl-drag arrowheads, colour choices, and timed fading. |
| 🧩 | **37 pointer styles** | 36 hands + laser dot, with a pinned toolbar and style picker. |
| 🪟 | **Real Wayland overlay** | Fullscreen `zwlr_layer_shell_v1` surface with `exclusive_zone(-1)` and a click-through input region. |
| ⌨️ | **Global shortcuts** | Built-in `Ctrl+Alt+H/X/E/M/S` through evdev, with optional WM fallback snippets. |
| 🔊 | **Tactile sound** | The original thocks, clicks, tocks, quacks, pews, and more via `paplay`. |
| 🧠 | **Persistent state** | Preferences, state file, `tinyhand://` URL commands, and a Unix control socket with Python-compatible output. |

---

## 📦 Install

```bash
git clone https://github.com/DuckB1t/tinyhand-linux.git
cd tinyhand-linux
./install.sh
```

The installer builds the release binaries and installs:

```text
~/.local/bin/tinyhand
~/.local/bin/tinyhandctl
~/.local/share/tinyhand-linux/assets/
```

It also installs desktop entries and registers the `tinyhand://` URL scheme. It does **not** touch Hyprland config, autostart, or keybinds.

### Requirements

- Rust and Cargo to build from source.
- A Wayland compositor with `zwlr_layer_shell_v1` — Hyprland, sway, river, labwc, wayfire, etc.
- Read access to `/dev/input/event*` for mouse buttons — usually membership in the `input` group.
- `paplay` for sounds — PulseAudio/PipeWire. Sounds are skipped quietly if unavailable.

---

## ▶️ Run

```bash
tinyhand
```

Optional launcher flags:

```bash
tinyhand --hidden     # start inactive, no welcome card
tinyhand --settings   # open settings
```

---

## 🎛️ Control it

```bash
tinyhandctl status
tinyhandctl on
tinyhandctl off
tinyhandctl toggle
tinyhandctl clear
tinyhandctl demo
tinyhandctl style laser
tinyhandctl style next
tinyhandctl sound on
tinyhandctl sound off
tinyhandctl settings
tinyhandctl section pointers
tinyhandctl toolbar
tinyhandctl quit
```

URL commands work too:

```text
tinyhand://toggle
tinyhand://style/coral
tinyhand://sound/off
tinyhand://clear
```

`tinyhandctl status` keeps the original Python output format, for example:

```text
active=True  style=lavender  sound=True  strokes=0  cursor=[0, 0]  settings=False  toolbar=False  reveal=0.0  show_toolbar=True  widgets=0  version=1.1.0
```

---

## 🕹️ Controls

| Gesture | Action |
|---|---|
| Move mouse | Hand follows the pointer |
| Left-click | Tap ripple + sound |
| Left-drag | Draw a line |
| Ctrl + left-drag | Draw an arrowhead |
| Right-click | Cycle pointer style |
| Hold Option, or configured interact key | Interact with the app underneath |
| Double-tap Shift | Toggle click-through |
| Esc | Exit / turn the pointer off |
| `Ctrl+Alt+H` | Toggle pointer |
| `Ctrl+Alt+X` | Clear drawing |
| `Ctrl+Alt+E` | Cycle style |
| `Ctrl+Alt+M` | Toggle sound |
| `Ctrl+Alt+S` | Open settings |

The last five are read globally through evdev when `/dev/input` is accessible. If not, copy the optional bindings from [`hyprland.conf.snippet`](hyprland.conf.snippet) or [`tacit.lua.snippet`](tacit.lua.snippet).

---

## 📚 Documentation

| Doc | What's inside |
|---|---|
| [`docs/REVERSE_ENGINEERING.md`](docs/REVERSE_ENGINEERING.md) | Upstream lifecycle, source map, constants, and REA findings. |
| [`docs/UI_REFERENCE.md`](docs/UI_REFERENCE.md) | Pixel-level UI and asset specification used by the port. |
| [`docs/INSTALL.md`](docs/INSTALL.md) | Detailed install, uninstall, and troubleshooting. |
| [`docs/PORTING_NOTES.md`](docs/PORTING_NOTES.md) | macOS → Wayland substitutions and known limitations. |
| [`docs/asset_reference.json`](docs/asset_reference.json) | Measured hand asset geometry. |

---

## 🧑‍💻 Development

```bash
make build        # cargo build --release
make test         # Rust unit tests
make test-python  # 27 offline reference-suite checks
make run          # run the Rust app from the source tree
make install      # build and install into ~/.local
```

Repository map:

```text
rust/src/
  app.rs          application state machine, event loop, commands
  ui.rs           toolbar / settings / welcome panels and hit-testing
  draw.rs         Cairo primitives, hand renderer, ink renderer
  wayland.rs      layer-shell surface, SHM buffers, input region
  input.rs        evdev mouse/keyboard + Hyprland IPC
  prefs.rs        preferences, 37 styles, ink and sound catalogues
  paths.rs        XDG paths
  cli.rs          tinyhand / tinyhandctl command line
src/tinyhand_linux/
  Python reference implementation kept as the parity oracle
tests/
  offline regression suite for the reference implementation
src/tinyhand_linux/assets/
  upstream hands, sounds, icon, geometry manifest
```

The Python implementation is kept for comparison and regression checks only; the installed app does not depend on it.

---

## 🙏 Credits

### Original work

This project exists because of **TinyHand** by **Kamran Ahmed** ([@nilbuild](https://x.com/nilbuild)).

- Website: [tinyhand.app](https://tinyhand.app)
- Upstream source: [github.com/nilbuild/tinyhand](https://github.com/nilbuild/tinyhand)
- Upstream licence: **MIT © 2026 Kamran Ahmed**
- Reverse-engineered commit: [`da98f98`](https://github.com/nilbuild/tinyhand/tree/da98f98) — v1.1.0

The original 36 hand renders, 15 tap sounds, menubar icon, UI copy, layout metrics, and behavior were preserved and reused under the upstream MIT licence. This port is not affiliated with or endorsed by the original author.

### Tools used to build the port

| Tool | Role |
|---|---|
| **REA** | Reverse-engineering workflow and target inspection. Apple asset-catalog/plist inspection requires a macOS host, so that limitation is documented honestly in the RE notes. |
| **Upstream Swift source** | Authoritative source for lifecycle, preferences, shortcuts, toolbar geometry, settings sections, and URL commands. |
| **Rust** | Native implementation with `wayland-client`, `wayland-protocols-wlr`, `cairo-rs`, `evdev`, `image`, `serde`, `serde_json`, `nix`, `memmap2`, `libc`, and `wait-timeout`. |
| **Python reference port** | Clean-room reference oracle for behavioral and pixel parity; not used at runtime. |
| **Pillow + NumPy** | Development-time asset measurement, alpha crops, and pixel checks. |
| **Hyprland IPC / `.socket2.sock`** | Live pointer position tracking. |
| **`grim`** | Screenshot verification on the live session. |
| **paplay / PipeWire** | Playing the original tap sounds. |
| **GitHub CLI** | Publishing and verifying this repository. |

Port maintained by **DuckB1t**.

---

## 🌍 Translations

Full technical documentation is in English above. These summaries cover the essentials.

<details>
<summary><strong>Español</strong></summary>

**TinyHand Linux** es un port nativo en Rust de TinyHand para Wayland, Hyprland y otros compositores wlroots. Reemplaza el cursor con una mano toy durante presentaciones y permite dibujar en pantalla.

```bash
git clone https://github.com/DuckB1t/tinyhand-linux.git
cd tinyhand-linux
./install.sh
```

Incluye las 36 manos y los 15 sonidos originales, tinta, barra de herramientas, ajustes y atajos `Ctrl+Alt+H/X/E/M/S`. El instalador **no** modifica la configuración de Hyprland, añade autostart ni atajos.

Créditos: TinyHand © 2026 Kamran Ahmed, MIT. Puerto realizado con Rust, Cairo, wayland-client, evdev y REA.

</details>

<details>
<summary><strong>Français</strong></summary>

**TinyHand Linux** est une version native en Rust pour Wayland/Hyprland de TinyHand. Elle remplace le curseur par une main jouet pendant vos présentations et permet de dessiner à l'écran.

```bash
git clone https://github.com/DuckB1t/tinyhand-linux.git
cd tinyhand-linux
./install.sh
```

Le projet conserve les 36 mains et les 15 sons d'origine, l'encre, la barre d'outils, les réglages et les raccourcis `Ctrl+Alt+H/X/E/M/S`. L'installateur ne touche **jamais** à la configuration Hyprland, au démarrage automatique ni aux raccourcis.

Crédits : TinyHand © 2026 Kamran Ahmed, MIT. Port réalisé avec Rust, Cairo, wayland-client, evdev et REA.

</details>

<details>
<summary><strong>Deutsch</strong></summary>

**TinyHand Linux** ist eine native Rust-Portierung von TinyHand für Wayland/Hyprland. Der Zeiger wird während Präsentationen durch eine Spielzeughand ersetzt; außerdem kannst du auf dem Bildschirm zeichnen.

```bash
git clone https://github.com/DuckB1t/tinyhand-linux.git
cd tinyhand-linux
./install.sh
```

Enthalten sind alle 36 Original-Hände, 15 Sounds, Freihand-Tinte, Symbolleiste, Einstellungen und die Shortcuts `Ctrl+Alt+H/X/E/M/S`. Das Installationsskript ändert **keine** Hyprland-Konfiguration, aktiviert keinen Autostart und legt keine Tastenkürzel an.

Credits: TinyHand © 2026 Kamran Ahmed, MIT. Portiert mit Rust, Cairo, wayland-client, evdev und REA.

</details>

<details>
<summary><strong>हिन्दी</strong></summary>

**TinyHand Linux**, TinyHand का एक देशी Rust पोर्ट है जो Wayland/Hyprland के लिए है। यह प्रेजेंटेशन के दौरान कर्सर को एक खिलौना हाथ से बदल देता है और स्क्रीन पर ड्रॉ करने की सुविधा देता है।

```bash
git clone https://github.com/DuckB1t/tinyhand-linux.git
cd tinyhand-linux
./install.sh
```

इसमें मूल 36 हाथ, 15 ध्वनियाँ, इंक, टूलबार, सेटिंग्स और `Ctrl+Alt+H/X/E/M/S` शॉर्टकट शामिल हैं। इंस्टॉलर **Hyprland कॉन्फ़िग, ऑटोस्टार्ट या कीबाइंड्स को बिल्कुल नहीं बदलता**।

क्रेडिट: TinyHand © 2026 Kamran Ahmed, MIT। पोर्ट Rust, Cairo, wayland-client, evdev और REA से बनाया गया।

</details>

<details>
<summary><strong>中文（简体）</strong></summary>

**TinyHand Linux** 是 TinyHand 的原生 Rust Wayland/Hyprland 移植版。演示时它会用卡通手替代鼠标指针，并支持在屏幕上自由绘制。

```bash
git clone https://github.com/DuckB1t/tinyhand-linux.git
cd tinyhand-linux
./install.sh
```

项目包含原始 36 款手部图像、15 种音效、墨迹绘制、工具栏、设置窗口，以及 `Ctrl+Alt+H/X/E/M/S` 快捷键。安装脚本**不会**修改 Hyprland 配置、不会启用开机自启，也不会添加按键绑定。

致谢：TinyHand © 2026 Kamran Ahmed，MIT 许可。移植使用 Rust、Cairo、wayland-client、evdev 和 REA 完成。

</details>

---

<div align="center">

**Made with care for the Wayland desktop.**<br>
Original art by [Kamran Ahmed](https://x.com/nilbuild) · Linux port by [DuckB1t](https://github.com/DuckB1t) · MIT licence.

</div>
