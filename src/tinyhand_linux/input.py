"""Mouse/keyboard and Hyprland pointer input for TinyHand Linux."""

from __future__ import annotations

import json
import os
import selectors
import socket
import subprocess
import time
from pathlib import Path
from typing import Callable

import evdev
from evdev import ecodes


BUTTON_NAMES = {
    ecodes.BTN_LEFT: "left",
    ecodes.BTN_RIGHT: "right",
    ecodes.BTN_MIDDLE: "middle",
    ecodes.BTN_SIDE: "side",
    ecodes.BTN_EXTRA: "extra",
}

MODIFIER_CODES = {
    ecodes.KEY_LEFTCTRL: "ctrl",
    ecodes.KEY_RIGHTCTRL: "ctrl",
    ecodes.KEY_LEFTALT: "alt",
    ecodes.KEY_RIGHTALT: "altgr",
    ecodes.KEY_LEFTSHIFT: "shift",
    ecodes.KEY_RIGHTSHIFT: "shift",
    ecodes.KEY_LEFTMETA: "super",
    ecodes.KEY_RIGHTMETA: "super",
}

CHAR_BASE = {
    ecodes.KEY_A: ("a", "A"), ecodes.KEY_B: ("b", "B"), ecodes.KEY_C: ("c", "C"),
    ecodes.KEY_D: ("d", "D"), ecodes.KEY_E: ("e", "E"), ecodes.KEY_F: ("f", "F"),
    ecodes.KEY_G: ("g", "G"), ecodes.KEY_H: ("h", "H"), ecodes.KEY_I: ("i", "I"),
    ecodes.KEY_J: ("j", "J"), ecodes.KEY_K: ("k", "K"), ecodes.KEY_L: ("l", "L"),
    ecodes.KEY_M: ("m", "M"), ecodes.KEY_N: ("n", "N"), ecodes.KEY_O: ("o", "O"),
    ecodes.KEY_P: ("p", "P"), ecodes.KEY_Q: ("q", "Q"), ecodes.KEY_R: ("r", "R"),
    ecodes.KEY_S: ("s", "S"), ecodes.KEY_T: ("t", "T"), ecodes.KEY_U: ("u", "U"),
    ecodes.KEY_V: ("v", "V"), ecodes.KEY_W: ("w", "W"), ecodes.KEY_X: ("x", "X"),
    ecodes.KEY_Y: ("y", "Y"), ecodes.KEY_Z: ("z", "Z"),
    ecodes.KEY_0: ("0", ")"), ecodes.KEY_1: ("1", "!"), ecodes.KEY_2: ("2", "@"),
    ecodes.KEY_3: ("3", "#"), ecodes.KEY_4: ("4", "$"), ecodes.KEY_5: ("5", "%"),
    ecodes.KEY_6: ("6", "^"), ecodes.KEY_7: ("7", "&"), ecodes.KEY_8: ("8", "*"),
    ecodes.KEY_9: ("9", "("), ecodes.KEY_SPACE: (" ", " "),
    ecodes.KEY_MINUS: ("-", "_"), ecodes.KEY_EQUAL: ("=", "+"),
    ecodes.KEY_LEFTBRACE: ("[", "{"), ecodes.KEY_RIGHTBRACE: ("]", "}"),
    ecodes.KEY_SEMICOLON: (";", ":"), ecodes.KEY_APOSTROPHE: ("'", '"'),
    ecodes.KEY_COMMA: (",", "<"), ecodes.KEY_DOT: (".", ">"),
    ecodes.KEY_SLASH: ("/", "?"), ecodes.KEY_BACKSLASH: ("\\", "|"),
    ecodes.KEY_GRAVE: ("`", "~"),
}

SPECIAL_KEYS = {
    ecodes.KEY_BACKSPACE: "BackSpace",
    ecodes.KEY_DELETE: "Delete",
    ecodes.KEY_ENTER: "Return",
    ecodes.KEY_KPENTER: "Return",
    ecodes.KEY_ESC: "Escape",
    ecodes.KEY_TAB: "Tab",
    ecodes.KEY_LEFT: "Left",
    ecodes.KEY_RIGHT: "Right",
    ecodes.KEY_UP: "Up",
    ecodes.KEY_DOWN: "Down",
    ecodes.KEY_HOME: "Home",
    ecodes.KEY_END: "End",
    ecodes.KEY_PAGEUP: "PageUp",
    ecodes.KEY_PAGEDOWN: "PageDown",
}
for _i in range(1, 13):
    SPECIAL_KEYS[getattr(ecodes, f"KEY_F{_i}")] = f"F{_i}"


class HyprlandCursor:
    """Tracks pointer position via Hyprland's event socket."""

    def __init__(self, on_move: Callable[[float, float], None]) -> None:
        self.on_move = on_move
        self.sock: socket.socket | None = None
        self.buffer = b""
        self.last_poll = 0.0
        runtime = Path(os.environ.get("XDG_RUNTIME_DIR", "/tmp"))
        signature = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE")
        if signature:
            path = runtime / "hypr" / signature / ".socket2.sock"
            if path.exists():
                try:
                    self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                    self.sock.connect(str(path))
                    self.sock.setblocking(False)
                except OSError:
                    self.sock = None

    def fileno(self) -> int | None:
        return self.sock.fileno() if self.sock else None

    def handle(self, _sock: socket.socket | None = None) -> None:
        if not self.sock:
            return
        try:
            data = self.sock.recv(65536)
        except (BlockingIOError, OSError):
            return
        if not data:
            return
        self.buffer += data
        while b"\n" in self.buffer:
            line, self.buffer = self.buffer.split(b"\n", 1)
            event = line.decode(errors="replace")
            if event.startswith("pointermove>>"):
                try:
                    x, y = event.split(">>", 1)[1].split(",")
                    self.on_move(float(x), float(y))
                except (ValueError, IndexError):
                    pass

    def poll(self) -> None:
        now = time.time()
        if now - self.last_poll < 0.05:
            return
        self.last_poll = now
        try:
            result = subprocess.run(
                ["hyprctl", "-j", "cursorpos"],
                capture_output=True,
                text=True,
                timeout=0.5,
            )
            if result.returncode == 0:
                data = json.loads(result.stdout)
                self.on_move(float(data["x"]), float(data["y"]))
        except (OSError, ValueError, KeyError, subprocess.TimeoutExpired):
            pass

    def close(self) -> None:
        if self.sock:
            try:
                self.sock.close()
            except OSError:
                pass


class InputManager:
    """Reads evdev devices for global buttons, wheel and UI keyboard input."""

    def __init__(
        self,
        on_button: Callable[[str, bool], None],
        on_wheel: Callable[[int, int], None],
        on_key: Callable[[str, set[str], int], None],
    ) -> None:
        self.on_button = on_button
        self.on_wheel = on_wheel
        self.on_key = on_key
        self.devices: list[evdev.InputDevice] = []
        self.modifiers: set[str] = set()
        self.last_button: dict[int, float] = {}

    def register(self, selector) -> None:
        devices: list[evdev.InputDevice] = []
        for path in sorted(Path("/dev/input").glob("event*")):
            try:
                dev = evdev.InputDevice(str(path))
                caps = dev.capabilities()
            except (OSError, PermissionError):
                continue
            keys = caps.get(ecodes.EV_KEY, [])
            rels = caps.get(ecodes.EV_REL, [])
            is_mouse = any(code in keys for code in BUTTON_NAMES) and (
                ecodes.REL_X in rels or ecodes.REL_Y in rels
            )
            is_keyboard = ecodes.KEY_A in keys and ecodes.KEY_ENTER in keys
            if is_mouse or is_keyboard:
                devices.append(dev)
        self.devices = devices
        for dev in devices:
            selector.register(dev.fd, selectors.EVENT_READ, ("input", dev))
        if devices:
            print(
                "[tinyhand] input:",
                ", ".join(f"{d.name}" for d in devices[:8]),
                flush=True,
            )
        else:
            print("[tinyhand] warning: no /dev/input devices available", flush=True)

    def handle(self, dev: evdev.InputDevice) -> None:
        try:
            events = list(dev.read())
        except (OSError, evdev.device.DeviceGrabError):
            return
        for event in events:
            if event.type == ecodes.EV_KEY:
                self._key(event.code, event.value)
            elif event.type == ecodes.EV_REL and event.value:
                if event.code == ecodes.REL_WHEEL:
                    self.on_wheel(0, -int(event.value))
                elif event.code == ecodes.REL_HWHEEL:
                    self.on_wheel(int(event.value), 0)

    def _key(self, code: int, value: int) -> None:
        if code in MODIFIER_CODES:
            name = MODIFIER_CODES[code]
            if value:
                self.modifiers.add(name)
            else:
                self.modifiers.discard(name)
            self.on_key(name, set(self.modifiers), value)
            return
        name = BUTTON_NAMES.get(code)
        if name and value in (0, 1):
            now = time.time()
            if now - self.last_button.get(code, 0) < 0.06:
                return
            self.last_button[code] = now
            self.on_button(name, bool(value))
            return
        if value != 1:
            return
        special = SPECIAL_KEYS.get(code)
        if special:
            self.on_key(special, set(self.modifiers), value)
            return
        pair = CHAR_BASE.get(code)
        if pair:
            self.on_key(pair[1] if "shift" in self.modifiers else pair[0], set(self.modifiers), value)

    def close(self) -> None:
        for dev in self.devices:
            try:
                dev.close()
            except OSError:
                pass
